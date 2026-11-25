use std::{
    future,
    io::{self, Cursor},
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use idna::uts46::{AsciiDenyList, DnsLength, Hyphens, Uts46};
use oasis_runtime_sdk::core::common::logger::get_logger;
use ptrie::Trie;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::{TcpListener, TcpStream},
    sync::{watch, OwnedSemaphorePermit, Semaphore},
    time::{timeout, timeout_at, Instant},
};
use tokio_rustls::TlsAcceptor;

use super::{
    idle::IdleTimeout,
    proxy_protocol, sni,
    tls::{CertificateProvisioner, CertificateProvisionerHandle, ACME_TLS_ALPN_PROTOCOL_ID},
};
use crate::http::tls;

/// Proxy operation mode.
#[derive(Debug, Default, Clone, Copy)]
pub enum Mode {
    /// Only forward encrypted TLS records based on initial SNI information.
    #[default]
    ForwardOnly,
    /// Terminate TLS, provision certificates via ACME and forward based on initial SNI
    /// information.
    TerminateTls,
}

/// Maximum value of a per-mapping timeout override. Longer overrides are clamped to this value.
pub const MAX_TIMEOUT: Duration = Duration::from_secs(24 * 3600);

/// Proxy configuration.
pub struct Config {
    pub listen_address: String,
    pub listen_port: u16,

    /// Maximum time to receive the PROXY protocol header (if any) and the TLS ClientHello and,
    /// when terminating TLS, to complete the TLS handshake.
    pub timeout_handshake: Duration,
    /// Maximum time to connect to the destination.
    pub timeout_connect: Duration,
    /// Maximum connection duration.
    pub timeout_connection: Duration,
    /// Maximum time without data being transferred in either direction.
    pub timeout_rw: Duration,

    pub max_connections: usize,

    /// Address of the hub that forwards connections to this proxy.
    ///
    /// When set, only the hub may connect and each connection must start with a PROXY protocol
    /// header.
    pub proxy_protocol_hub: Option<IpAddr>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_address: "0.0.0.0".to_string(),
            listen_port: 443,
            timeout_handshake: Duration::from_secs(1),
            timeout_connect: Duration::from_secs(1),
            timeout_connection: Duration::from_secs(45),
            timeout_rw: Duration::from_secs(30),
            max_connections: 1024,
            proxy_protocol_hub: None,
        }
    }
}

/// Per-mapping timeout overrides.
///
/// Timeouts that are not set fall back to the proxy-wide values from [`Config`]. The handshake
/// timeout cannot be overridden as it applies before the mapping is known.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Timeouts {
    /// Maximum time to connect to the destination.
    pub connect: Option<Duration>,
    /// Maximum connection duration.
    pub connection: Option<Duration>,
    /// Maximum time without data being transferred in either direction.
    pub rw: Option<Duration>,
}

/// A group of connections that can share a concurrency limit and be closed together.
///
/// Clones refer to the same group so it can be shared by multiple mappings.
#[derive(Clone, Debug)]
pub struct ConnectionGroup(Arc<ConnectionGroupInner>);

#[derive(Debug)]
struct ConnectionGroupInner {
    limit: Option<Arc<Semaphore>>,
    closed: watch::Sender<bool>,
}

impl ConnectionGroup {
    /// Create a new connection group, optionally limiting the number of concurrent connections.
    pub fn new(max_connections: Option<usize>) -> Self {
        Self(Arc::new(ConnectionGroupInner {
            limit: max_connections.map(|max| Arc::new(Semaphore::new(max))),
            closed: watch::Sender::new(false),
        }))
    }

    /// Close all connections in the group and reject any new ones.
    pub fn close(&self) {
        self.0.closed.send_replace(true);
    }

    /// Add a connection to the group without waiting for a free slot.
    fn join(&self) -> Result<ConnectionGroupMember> {
        // Subscribe before checking so that a concurrent close is not missed.
        let closed = self.0.closed.subscribe();
        if *closed.borrow() {
            return Err(anyhow!("connection group closed"));
        }
        let permit = match &self.0.limit {
            Some(limit) => Some(
                limit
                    .clone()
                    .try_acquire_owned()
                    .map_err(|_| anyhow!("connection limit reached"))?,
            ),
            None => None,
        };

        Ok(ConnectionGroupMember {
            _permit: permit,
            closed,
        })
    }
}

/// Membership of a connection in a group. The connection slot is held until dropped.
struct ConnectionGroupMember {
    _permit: Option<OwnedSemaphorePermit>,
    closed: watch::Receiver<bool>,
}

impl ConnectionGroupMember {
    /// Wait until the group is closed.
    async fn closed(&mut self) {
        if self.closed.wait_for(|closed| *closed).await.is_err() {
            // The group can no longer be closed.
            future::pending().await
        }
    }
}

/// Proxy mapping.
#[derive(Clone, Default, Debug)]
pub struct Mapping {
    /// Domain name to map (based on SNI extension). Names are matched case-insensitively.
    pub name: String,
    /// Destination address to connect to.
    pub dst_address: String,
    /// Destination port to connect to.
    pub dst_port: u16,
    /// Proxy mode for this mapping.
    pub mode: Mode,
    /// Whether to send a PROXY protocol header to the destination.
    pub proxy_protocol: bool,
    /// Timeout overrides for this mapping.
    pub timeouts: Timeouts,
    /// Optional group that connections for this mapping belong to.
    pub connection_group: Option<ConnectionGroup>,
}

/// Proxy mapping set.
struct Mappings {
    tree: Mutex<Trie<String, Arc<Mapping>>>,
}

impl Mappings {
    /// Create a new empty proxy mapping set.
    fn new() -> Self {
        Self {
            tree: Mutex::new(Trie::new()),
        }
    }

    /// Add a new proxy mapping, storing it under the canonical form of its name.
    ///
    /// If a mapping for the same domain exists, it is overwritten. Returns the stored mapping.
    fn add(&self, mut mapping: Mapping) -> Result<Arc<Mapping>> {
        mapping.name = canonical_name(&mapping.name)
            .ok_or_else(|| anyhow!("invalid domain name ({})", mapping.name))?;
        let mapping = Arc::new(mapping);
        let mut tree = self.tree.lock().unwrap();
        tree.insert(reverse(&mapping.name), mapping.clone());
        Ok(mapping)
    }

    /// Remove an existing proxy mapping, returning it.
    fn remove(&self, name: &str) -> Option<Arc<Mapping>> {
        let name = canonical_name(name)?;
        let mut tree = self.tree.lock().unwrap();
        tree.remove(reverse(&name))
    }

    /// Lookup an existing proxy mapping by using the longest suffix match.
    fn get(&self, name: &str) -> Option<Arc<Mapping>> {
        let name = canonical_name(name)?;
        let tree = self.tree.lock().unwrap();
        tree.find_longest_prefix(reverse(&name)).cloned()
    }
}

/// Canonical form of a DNS name, used whenever names are compared.
///
/// This is the lowercase IDNA ASCII form (UTS #46, non-transitional), the same form that DNS
/// resolution and SNI use (RFC 6066), so e.g. `Bücher.example.org` and `xn--bcher-kva.example.org`
/// are the same name. A trailing dot for the root zone is removed as SNI carries names without it.
///
/// Returns `None` for names that cannot be converted or have empty labels (e.g. `a..b` or `a..`),
/// which ensures that canonicalizing a canonical name does not change it.
pub fn canonical_name(name: &str) -> Option<String> {
    let name = Uts46::new()
        .to_ascii(
            name.as_bytes(),
            AsciiDenyList::EMPTY,
            Hyphens::Allow,
            DnsLength::Ignore,
        )
        .ok()?;
    // Conversion is done first as it can map other characters (e.g. U+3002) to dots.
    let name = name.strip_suffix('.').unwrap_or(&name);
    if name.split('.').any(str::is_empty) {
        return None;
    }
    Some(name.to_string())
}

/// Resolve a timeout override, falling back to the given default.
fn effective_timeout(timeout: Option<Duration>, default: Duration) -> Duration {
    timeout.unwrap_or(default).min(MAX_TIMEOUT)
}

/// Convert a given domain name into a reversed vector of domain atoms.
fn reverse(name: &str) -> impl Iterator<Item = String> + '_ {
    name.split('.').rev().map(|s| s.to_string())
}

struct State {
    cfg: Config,
    mappings: Mappings,
    provisioner: CertificateProvisionerHandle,
    acceptor: TlsAcceptor,
}

/// Proxy.
pub struct Proxy {
    rt: tokio::runtime::Runtime,
    state: Arc<State>,
    handle: ProxyHandle,
    provisioner: Option<CertificateProvisioner>,
}

impl Proxy {
    /// Create a new proxy instance.
    pub fn new(cfg: Config, acme: tls::AcmeAccount) -> Result<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .thread_name("rofl-proxy")
            .enable_all()
            .build()?;

        let provisioner = CertificateProvisioner::new(acme);
        let state = State {
            cfg,
            mappings: Mappings::new(),
            provisioner: provisioner.handle().clone(),
            acceptor: TlsAcceptor::from(provisioner.server_config(false)),
        };
        let state = Arc::new(state);

        Ok(Self {
            rt,
            state: state.clone(),
            handle: ProxyHandle { state },
            provisioner: Some(provisioner),
        })
    }

    /// Proxy handle that can be used to update proxy mappings.
    pub fn handle(&self) -> &ProxyHandle {
        &self.handle
    }

    /// Add a new proxy mapping.
    pub async fn add_mapping(&self, mapping: Mapping) -> Result<()> {
        self.handle.add_mapping(mapping).await
    }

    /// Remove an existing proxy mapping.
    pub async fn remove_mapping(&self, name: &str) {
        self.handle.remove_mapping(name).await
    }

    /// Lookup an existing proxy mapping.
    pub fn get_mapping(&self, name: &str) -> Option<Arc<Mapping>> {
        self.handle.get_mapping(name)
    }

    /// Start the proxy.
    pub fn start(&mut self) {
        match self.provisioner.take() {
            Some(provisioner) => {
                self.rt.spawn(async move { provisioner.start() });
            }
            None => {
                // Prevent startup more than once.
                return;
            }
        }

        let state = self.state.clone();

        self.rt.spawn(async {
            let logger = get_logger("proxy/http");

            let result = Self::run(state).await;
            if let Err(err) = result {
                slog::error!(logger, "failed to run proxy"; "err" => ?err);
            }
        });
    }

    async fn run(state: Arc<State>) -> Result<()> {
        let listener =
            TcpListener::bind((state.cfg.listen_address.clone(), state.cfg.listen_port)).await?;
        let connection_semaphore = Arc::new(Semaphore::new(state.cfg.max_connections));

        loop {
            let connection_permit = connection_semaphore.clone().acquire_owned().await.unwrap();
            match listener.accept().await {
                Ok((stream, peer)) => {
                    let state = state.clone();

                    tokio::spawn(async move {
                        let logger = get_logger("proxy/http");

                        if let Err(err) = Self::handle_connection(state, stream, peer).await {
                            slog::error!(logger, "failed to handle connection"; "err" => ?err);
                        }
                        drop(connection_permit);
                    });
                }
                Err(_) => continue,
            }
        }
    }

    async fn handle_connection(
        state: Arc<State>,
        mut stream: TcpStream,
        peer: SocketAddr,
    ) -> Result<()> {
        let start = Instant::now();
        // Only proxy-wide limits can apply until the mapping is known.
        let timeout_handshake = state
            .cfg
            .timeout_handshake
            .min(state.cfg.timeout_connection);
        let handshake_deadline = start + timeout_handshake;

        let (proxy_addrs, tls_hello) = timeout_at(handshake_deadline, async {
            // The hub sends a PROXY protocol header before the TLS handshake.
            let proxy_addrs = match state.cfg.proxy_protocol_hub {
                Some(hub) if hub.to_canonical() == peer.ip().to_canonical() => Some(
                    proxy_protocol::read(&mut stream)
                        .await
                        .context("failed to read PROXY protocol header")?,
                ),
                Some(_) => return Err(anyhow!("connection not from hub ({})", peer.ip())),
                None => None,
            };
            let tls_hello = Self::parse_tls_hello(&mut stream)
                .await
                .context("failed to parse TLS hello")?;
            Ok((proxy_addrs, tls_hello))
        })
        .await
        .context("TLS handshake timeout")??;

        // Resolve destination.
        let mapping = state
            .mappings
            .get(&tls_hello.sni)
            .ok_or(anyhow!("unknown host ({})", tls_hello.sni))?;

        // Join the connection group (if any), holding a slot until the connection is closed.
        let mut member = mapping
            .connection_group
            .as_ref()
            .map(ConnectionGroup::join)
            .transpose()?;
        let group_closed = async {
            match member.as_mut() {
                Some(member) => member.closed().await,
                None => future::pending().await,
            }
        };

        // Prepare the PROXY protocol header for the destination, forwarding the addresses received
        // from the hub when available.
        let proxy_header = match (mapping.proxy_protocol, proxy_addrs) {
            (false, _) => None,
            (true, Some((src, dst))) => Some(proxy_protocol::encode(src, dst)),
            (true, None) => Some(proxy_protocol::encode(peer, stream.local_addr()?)),
        };

        // The maximum connection duration also includes the time spent on the handshake.
        let timeout_connection =
            effective_timeout(mapping.timeouts.connection, state.cfg.timeout_connection);
        let connection = timeout_at(
            start + timeout_connection,
            Self::handle_mapped_connection(
                state,
                stream,
                mapping,
                tls_hello,
                proxy_header,
                handshake_deadline,
            ),
        );

        tokio::select! {
            result = connection => result.context("max connection time limit reached")?,
            _ = group_closed => Err(anyhow!("connection group closed")),
        }
    }

    async fn handle_mapped_connection(
        state: Arc<State>,
        mut stream: TcpStream,
        mapping: Arc<Mapping>,
        tls_hello: TlsHello,
        proxy_header: Option<Vec<u8>>,
        handshake_deadline: Instant,
    ) -> Result<()> {
        let timeout_connect =
            effective_timeout(mapping.timeouts.connect, state.cfg.timeout_connect);
        let timeout_rw = effective_timeout(mapping.timeouts.rw, state.cfg.timeout_rw);

        let connect_to_destination = async || -> Result<_> {
            let dst = timeout(
                timeout_connect,
                TcpStream::connect((mapping.dst_address.clone(), mapping.dst_port)),
            )
            .await
            .context(format!(
                "connect to destination ({}) timeout",
                mapping.dst_address
            ))?
            .context(format!(
                "failed to connect to destination ({})",
                mapping.dst_address
            ))?;
            Ok(dst)
        };

        match mapping.mode {
            Mode::ForwardOnly => {
                // Replay TLS handshake buffer and forward.
                let mut dst = connect_to_destination().await?;
                match proxy_header {
                    // Send the header in the same write as two small writes delay the second one.
                    Some(header) => dst.write_all(&[header, tls_hello.raw].concat()).await?,
                    None => dst.write_all(&tls_hello.raw).await?,
                }
                Self::handle_forwarding(&mut stream, &mut dst, timeout_rw).await?;
            }
            Mode::TerminateTls => {
                // Handle TLS termination. The TLS handshake must also complete in time as the
                // connection is not yet subject to the idle timeout.
                let accept = state.acceptor.accept_with(stream, |conn| {
                    // Replay TLS handshake buffer.
                    let mut cursor = Cursor::new(&tls_hello.raw);
                    while cursor.position() < cursor.get_ref().len() as u64 {
                        match conn.read_tls(&mut cursor) {
                            Ok(count) => {
                                if conn.process_new_packets().is_err() {
                                    break;
                                }
                                if count == 0 {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                });
                let mut stream = timeout_at(handshake_deadline, accept)
                    .await
                    .context("TLS handshake timeout")??;

                // Skip further connection processing in case of ACME-TLS ALPN protocol.
                if stream.get_ref().1.alpn_protocol() == Some(ACME_TLS_ALPN_PROTOCOL_ID) {
                    return Ok(());
                }

                let mut dst = connect_to_destination().await?;
                if let Some(header) = proxy_header {
                    dst.write_all(&header).await?;
                }
                Self::handle_forwarding(&mut stream, &mut dst, timeout_rw).await?;
            }
        }

        Ok(())
    }

    async fn handle_forwarding<A, B>(a: &mut A, b: &mut B, timeout_rw: Duration) -> Result<()>
    where
        A: AsyncRead + AsyncWrite + Unpin,
        B: AsyncRead + AsyncWrite + Unpin,
    {
        // Close the connection when no data is transferred in either direction for too long.
        let idle = IdleTimeout::new(timeout_rw);
        let mut a = idle.track(a);
        let mut b = idle.track(b);

        tokio::select! {
            result = tokio::io::copy_bidirectional(&mut a, &mut b) => {
                result?;
                Ok(())
            }
            _ = idle.expired() => Err(io::Error::from(io::ErrorKind::TimedOut).into()),
        }
    }

    async fn parse_tls_hello(stream: &mut TcpStream) -> Result<TlsHello> {
        let mut raw = vec![0u8; sni::TLS_MAX_RECORD_SIZE];
        let mut buffer = ReadBuf::new(&mut raw);

        loop {
            let count = stream.read_buf(&mut buffer).await?;
            if count == 0 {
                break;
            }

            match sni::parse(buffer.filled()) {
                Ok(Some(sni)) => {
                    let len = buffer.filled().len();
                    raw.truncate(len);

                    return Ok(TlsHello { sni, raw });
                }
                Ok(None) => {
                    continue;
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }

        Err(anyhow!("missing SNI extension"))
    }
}

#[derive(Clone)]
pub struct ProxyHandle {
    state: Arc<State>,
}

impl ProxyHandle {
    /// Add a new proxy mapping.
    ///
    /// The mapping is stored under the canonical form of its name (see [`canonical_name`]). Fails
    /// if the name is not a valid domain name.
    pub async fn add_mapping(&self, mapping: Mapping) -> Result<()> {
        let mapping = self.state.mappings.add(mapping)?;
        // Certificates are also provisioned for the canonical name, as TLS looks them up by the
        // lowercase SNI.
        if matches!(mapping.mode, Mode::TerminateTls) {
            self.state.provisioner.add_domain(&mapping.name).await;
        }
        Ok(())
    }

    /// Remove an existing proxy mapping.
    pub async fn remove_mapping(&self, name: &str) {
        let mapping = self.state.mappings.remove(name);
        if let Some(mapping) = mapping {
            if matches!(mapping.mode, Mode::TerminateTls) {
                self.state.provisioner.remove_domain(&mapping.name).await;
            }
        }
    }

    /// Lookup an existing proxy mapping.
    pub fn get_mapping(&self, name: &str) -> Option<Arc<Mapping>> {
        self.state.mappings.get(name)
    }
}

/// Information extracted from a TLS ClientHello message.
struct TlsHello {
    /// Extracted value of the SNI extension.
    sni: String,
    /// Raw data read to parse the TLS record.
    raw: Vec<u8>,
}

#[cfg(test)]
mod test {
    use base64::prelude::*;

    use super::*;
    use crate::http::sni::test::RECORD_SNI;

    /// Create an ACME account that does not need network access.
    ///
    /// The credentials include the directory URLs so that the directory is not fetched.
    fn acme_account() -> tls::AcmeAccount {
        let (_, key) = instant_acme::Key::generate_pkcs8().unwrap();
        let credentials = serde_json::from_value(serde_json::json!({
            "id": "",
            "key_pkcs8": BASE64_URL_SAFE_NO_PAD.encode(key.secret_pkcs8_der()),
            "urls": {"newNonce": "", "newAccount": "", "newOrder": ""},
        }))
        .unwrap();
        let builder = tls::AcmeAccount::builder().unwrap();
        tokio_test::block_on(builder.from_credentials(credentials)).unwrap()
    }

    /// Send the given data through a proxy with a single mapping.
    ///
    /// Returns the client address, the proxy address and the data received by the destination.
    fn forward(
        hub: Option<IpAddr>,
        proxy_protocol: bool,
        data: &[u8],
    ) -> Result<(SocketAddr, SocketAddr, Vec<u8>)> {
        let cfg = Config {
            proxy_protocol_hub: hub,
            ..Default::default()
        };
        let proxy = Proxy::new(cfg, acme_account())?;

        proxy.rt.block_on(async {
            let listener = TcpListener::bind("127.0.0.1:0").await?;
            let destination = TcpListener::bind("127.0.0.1:0").await?;
            proxy
                .state
                .mappings
                .add(Mapping {
                    name: "example.net".to_string(),
                    dst_address: "127.0.0.1".to_string(),
                    dst_port: destination.local_addr()?.port(),
                    mode: Mode::ForwardOnly,
                    proxy_protocol,
                    ..Default::default()
                })
                .unwrap();

            let mut client = TcpStream::connect(listener.local_addr()?).await?;
            client.write_all(data).await?;
            client.shutdown().await?;
            let (stream, peer) = listener.accept().await?;
            let local = stream.local_addr()?;

            let received = tokio::spawn(async move {
                let (mut stream, _) = destination.accept().await?;
                let mut data = vec![];
                stream.read_to_end(&mut data).await?;
                anyhow::Ok(data)
            });
            Proxy::handle_connection(proxy.state.clone(), stream, peer).await?;

            Ok((peer, local, received.await??))
        })
    }

    /// Open a connection through the proxy that remains idle after the TLS hello is forwarded.
    ///
    /// Returns the task handling the connection, the client stream and the destination stream.
    async fn open_idle_connection(
        state: &Arc<State>,
        listener: &TcpListener,
        destination: &TcpListener,
    ) -> Result<(tokio::task::JoinHandle<Result<()>>, TcpStream, TcpStream)> {
        let mut client = TcpStream::connect(listener.local_addr()?).await?;
        client.write_all(RECORD_SNI).await?;
        let (stream, peer) = listener.accept().await?;
        let handle = tokio::spawn(Proxy::handle_connection(state.clone(), stream, peer));
        let (mut dst, _) = timeout(Duration::from_secs(10), destination.accept()).await??;
        let mut hello = vec![0; RECORD_SNI.len()];
        dst.read_exact(&mut hello).await?;
        Ok((handle, client, dst))
    }

    /// Open a connection through the proxy, returning the error it was rejected with.
    async fn open_rejected_connection(
        state: &Arc<State>,
        listener: &TcpListener,
    ) -> Result<anyhow::Error> {
        let mut client = TcpStream::connect(listener.local_addr()?).await?;
        client.write_all(RECORD_SNI).await?;
        let (stream, peer) = listener.accept().await?;
        let result = timeout(
            Duration::from_secs(10),
            Proxy::handle_connection(state.clone(), stream, peer),
        )
        .await?;
        Ok(result.unwrap_err())
    }

    /// Create proxy state that terminates TLS using a self-signed certificate for the test SNI.
    fn tls_state(cfg: Config) -> Arc<State> {
        let cert = rcgen::generate_simple_self_signed(vec!["example.net".to_string()]).unwrap();
        let key = rustls::pki_types::PrivateKeyDer::Pkcs8(cert.signing_key.serialize_der().into());
        let server_cfg = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert.cert.der().clone()], key)
            .unwrap();
        let provisioner = CertificateProvisioner::new(acme_account());

        Arc::new(State {
            cfg,
            mappings: Mappings::new(),
            provisioner: provisioner.handle().clone(),
            acceptor: TlsAcceptor::from(Arc::new(server_cfg)),
        })
    }

    #[test]
    fn test_handshake_deadline() {
        let cfg = Config {
            timeout_handshake: Duration::from_secs(60),
            timeout_connection: Duration::from_millis(100),
            ..Default::default()
        };
        let proxy = Proxy::new(cfg, acme_account()).unwrap();

        proxy
            .rt
            .block_on(async {
                let listener = TcpListener::bind("127.0.0.1:0").await?;

                // Waiting for the ClientHello should be bounded by the connection timeout.
                let _client = TcpStream::connect(listener.local_addr()?).await?;
                let (stream, peer) = listener.accept().await?;
                let err = timeout(
                    Duration::from_secs(10),
                    Proxy::handle_connection(proxy.state.clone(), stream, peer),
                )
                .await?
                .unwrap_err();
                assert_eq!(err.to_string(), "TLS handshake timeout");

                anyhow::Ok(())
            })
            .unwrap();
    }

    #[test]
    fn test_tls_handshake_deadline() {
        let state = tls_state(Config {
            timeout_handshake: Duration::from_millis(200),
            ..Default::default()
        });
        state
            .mappings
            .add(Mapping {
                name: "example.net".to_string(),
                dst_address: "127.0.0.1".to_string(),
                dst_port: 1,
                mode: Mode::TerminateTls,
                timeouts: Timeouts {
                    connection: Some(Duration::from_secs(3600)),
                    ..Default::default()
                },
                ..Default::default()
            })
            .unwrap();

        tokio_test::block_on(async {
            let listener = TcpListener::bind("127.0.0.1:0").await?;

            // A client that stalls the TLS handshake should be closed at the handshake deadline
            // instead of after the maximum connection duration.
            let mut client = TcpStream::connect(listener.local_addr()?).await?;
            client.write_all(RECORD_SNI).await?;
            let (stream, peer) = listener.accept().await?;
            let err = timeout(
                Duration::from_secs(10),
                Proxy::handle_connection(state.clone(), stream, peer),
            )
            .await?
            .unwrap_err();
            assert_eq!(err.to_string(), "TLS handshake timeout");

            anyhow::Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_idle_timeout() {
        let cfg = Config {
            timeout_rw: Duration::from_millis(300),
            ..Default::default()
        };
        let proxy = Proxy::new(cfg, acme_account()).unwrap();

        proxy
            .rt
            .block_on(async {
                let listener = TcpListener::bind("127.0.0.1:0").await?;
                let destination = TcpListener::bind("127.0.0.1:0").await?;
                proxy
                    .state
                    .mappings
                    .add(Mapping {
                        name: "example.net".to_string(),
                        dst_address: "127.0.0.1".to_string(),
                        dst_port: destination.local_addr()?.port(),
                        ..Default::default()
                    })
                    .unwrap();

                for server_to_client in [true, false] {
                    let (handle, mut client, mut dst) =
                        open_idle_connection(&proxy.state, &listener, &destination).await?;
                    let (sender, receiver) = match server_to_client {
                        true => (&mut dst, &mut client),
                        false => (&mut client, &mut dst),
                    };

                    // Data flowing in a single direction should keep the connection open for
                    // longer than the idle timeout.
                    let mut data = [0u8; 1];
                    for _ in 0..10 {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        sender.write_all(b"x").await?;
                        receiver.read_exact(&mut data).await?;
                    }
                    assert!(!handle.is_finished());

                    // The connection should be closed once no more data is transferred.
                    let err = timeout(Duration::from_secs(10), handle)
                        .await??
                        .unwrap_err();
                    assert_eq!(err.downcast::<io::Error>()?.kind(), io::ErrorKind::TimedOut);
                }

                anyhow::Ok(())
            })
            .unwrap();
    }

    #[test]
    fn test_mapping_timeouts() {
        let proxy = Proxy::new(Config::default(), acme_account()).unwrap();

        proxy
            .rt
            .block_on(async {
                let listener = TcpListener::bind("127.0.0.1:0").await?;
                let destination = TcpListener::bind("127.0.0.1:0").await?;
                let dst_port = destination.local_addr()?.port();
                let add_mapping = |timeouts| {
                    proxy
                        .state
                        .mappings
                        .add(Mapping {
                            name: "example.net".to_string(),
                            dst_address: "127.0.0.1".to_string(),
                            dst_port,
                            timeouts,
                            ..Default::default()
                        })
                        .unwrap()
                };
                // Much lower than the defaults so that the overrides are what closes connections.
                let short = Some(Duration::from_millis(100));
                let deadline = Duration::from_secs(10);

                // An idle connection should be closed after the read/write timeout.
                add_mapping(Timeouts {
                    rw: short,
                    ..Default::default()
                });
                let (handle, _client, _dst) =
                    open_idle_connection(&proxy.state, &listener, &destination).await?;
                let err = timeout(deadline, handle).await??.unwrap_err();
                assert_eq!(err.downcast::<io::Error>()?.kind(), io::ErrorKind::TimedOut);

                // A connection should be closed after the maximum connection duration.
                add_mapping(Timeouts {
                    connection: short,
                    ..Default::default()
                });
                let (handle, _client, _dst) =
                    open_idle_connection(&proxy.state, &listener, &destination).await?;
                let err = timeout(deadline, handle).await??.unwrap_err();
                assert_eq!(err.to_string(), "max connection time limit reached");

                anyhow::Ok(())
            })
            .unwrap();
    }

    #[test]
    fn test_mapping_connection_group() {
        let proxy = Proxy::new(Config::default(), acme_account()).unwrap();

        proxy
            .rt
            .block_on(async {
                let listener = TcpListener::bind("127.0.0.1:0").await?;
                let destination = TcpListener::bind("127.0.0.1:0").await?;
                let dst_port = destination.local_addr()?.port();
                let add_mapping = |group: &ConnectionGroup| {
                    proxy
                        .state
                        .mappings
                        .add(Mapping {
                            name: "example.net".to_string(),
                            dst_address: "127.0.0.1".to_string(),
                            dst_port,
                            connection_group: Some(group.clone()),
                            ..Default::default()
                        })
                        .unwrap()
                };
                let deadline = Duration::from_secs(10);

                let group = ConnectionGroup::new(Some(1));
                add_mapping(&group);

                // The first connection occupies the only slot.
                let (handle, client, dst) =
                    open_idle_connection(&proxy.state, &listener, &destination).await?;

                // Further connections should be rejected while the slot is occupied.
                let err = open_rejected_connection(&proxy.state, &listener).await?;
                assert_eq!(err.to_string(), "connection limit reached");

                // The slot should be released once the first connection is closed.
                drop(client);
                drop(dst);
                let _ = timeout(deadline, handle).await?;
                let (handle, _client, _dst) =
                    open_idle_connection(&proxy.state, &listener, &destination).await?;

                // Closing the group should close its connections and reject new ones.
                group.close();
                let err = timeout(deadline, handle).await??.unwrap_err();
                assert_eq!(err.to_string(), "connection group closed");
                let err = open_rejected_connection(&proxy.state, &listener).await?;
                assert_eq!(err.to_string(), "connection group closed");

                // A mapping with a new group should accept connections again.
                add_mapping(&ConnectionGroup::new(Some(1)));
                let (handle, _client, _dst) =
                    open_idle_connection(&proxy.state, &listener, &destination).await?;
                handle.abort();

                anyhow::Ok(())
            })
            .unwrap();
    }

    #[test]
    fn test_proxy_protocol() {
        let hub = Some("127.0.0.1".parse().unwrap());
        let header = proxy_protocol::encode(
            "192.0.2.1:123".parse().unwrap(),
            "192.0.2.2:5678".parse().unwrap(),
        );
        let hello = RECORD_SNI;
        let header_hello = [&header[..], hello].concat();

        // Without the PROXY protocol the data should be forwarded unchanged.
        let (_, _, data) = forward(None, false, hello).unwrap();
        assert_eq!(data, hello);

        // A header with the connection addresses should be sent to the destination.
        let (client, proxy, data) = forward(None, true, hello).unwrap();
        let expected = [&proxy_protocol::encode(client, proxy)[..], hello].concat();
        assert_eq!(data, expected);

        // A header received from the hub should be forwarded to the destination.
        let (_, _, data) = forward(hub, true, &header_hello).unwrap();
        assert_eq!(data, header_hello);

        // A header received from the hub should not be forwarded when not enabled.
        let (_, _, data) = forward(hub, false, &header_hello).unwrap();
        assert_eq!(data, hello);

        // A connection from the hub without a header should be rejected.
        let err = forward(hub, true, hello).unwrap_err();
        assert_eq!(err.to_string(), "failed to read PROXY protocol header");

        // A connection that is not from the hub should be rejected.
        let hub = Some("127.0.0.2".parse().unwrap());
        let err = forward(hub, true, &header_hello).unwrap_err();
        assert_eq!(err.to_string(), "connection not from hub (127.0.0.1)");
    }

    #[test]
    fn test_mappings() {
        let mappings = Mappings::new();
        let mapping = mappings.get("example.com");
        assert!(mapping.is_none());

        mappings
            .add(Mapping {
                name: "foo.example.com".to_string(),
                dst_address: "a".to_string(),
                dst_port: 1234,
                mode: Mode::ForwardOnly,
                proxy_protocol: false,
                ..Default::default()
            })
            .unwrap();
        mappings
            .add(Mapping {
                name: "bar.foo.example.com".to_string(),
                dst_address: "b".to_string(),
                dst_port: 1234,
                mode: Mode::ForwardOnly,
                proxy_protocol: false,
                ..Default::default()
            })
            .unwrap();
        mappings
            .add(Mapping {
                name: "another.example.com".to_string(),
                dst_address: "c".to_string(),
                dst_port: 1234,
                mode: Mode::ForwardOnly,
                proxy_protocol: false,
                ..Default::default()
            })
            .unwrap();

        // Top-level should not exist.
        let mapping = mappings.get("example.com");
        assert!(mapping.is_none());

        // Direct mapping should exist.
        let mapping = mappings
            .get("foo.example.com")
            .expect("mapping should exist");
        assert_eq!(mapping.name, "foo.example.com");
        assert_eq!(mapping.dst_address, "a");

        // Any suffix should exist.
        let mapping = mappings
            .get("my.custom.subdomain.foo.example.com")
            .expect("mapping should exist");
        assert_eq!(mapping.name, "foo.example.com");
        assert_eq!(mapping.dst_address, "a");

        // Longest suffix should be used.
        let mapping = mappings
            .get("my.custom.bar.foo.example.com")
            .expect("mapping should exist");
        assert_eq!(mapping.name, "bar.foo.example.com");
        assert_eq!(mapping.dst_address, "b");

        // Names should be matched case-insensitively and regardless of a trailing dot.
        let mapping = mappings
            .get("My.Custom.FOO.example.com.")
            .expect("mapping should exist");
        assert_eq!(mapping.name, "foo.example.com");

        // A mapping with a differently written name should replace the existing one.
        mappings
            .add(Mapping {
                name: "Another.Example.COM.".to_string(),
                dst_address: "d".to_string(),
                ..Default::default()
            })
            .unwrap();
        let mapping = mappings
            .get("another.example.com")
            .expect("mapping should exist");
        assert_eq!(mapping.name, "another.example.com");
        assert_eq!(mapping.dst_address, "d");

        // Removal should also match differently written names.
        assert!(mappings.remove("ANOTHER.example.com").is_some());
        assert!(mappings.get("another.example.com").is_none());

        // Names with empty labels should be rejected and never match.
        for name in [
            "docs.example..",
            "docs..example.com",
            ".example.com",
            "",
            ".",
        ] {
            let result = mappings.add(Mapping {
                name: name.to_string(),
                ..Default::default()
            });
            assert!(result.is_err(), "{name:?} should be rejected");
            assert!(mappings.remove(name).is_none());
            assert!(mappings.get(name).is_none());
        }
        assert!(mappings.get("docs.example").is_none());

        // Unicode and ASCII spellings of an internationalized name should be the same route.
        mappings
            .add(Mapping {
                name: "xn--bcher-kva.example.org".to_string(),
                dst_address: "old".to_string(),
                ..Default::default()
            })
            .unwrap();
        mappings
            .add(Mapping {
                name: "Bücher.example.org".to_string(),
                dst_address: "new".to_string(),
                ..Default::default()
            })
            .unwrap();
        for name in ["xn--bcher-kva.example.org", "bücher.example.org"] {
            let mapping = mappings.get(name).expect("mapping should exist");
            assert_eq!(mapping.name, "xn--bcher-kva.example.org");
            assert_eq!(mapping.dst_address, "new");
        }
        assert!(mappings.remove("bücher.example.org").is_some());
        assert!(mappings.get("xn--bcher-kva.example.org").is_none());
    }

    #[test]
    fn test_canonical_name() {
        let canonical = |name: &str| canonical_name(name).unwrap();
        assert_eq!(canonical("app.example.org"), "app.example.org");
        assert_eq!(canonical("App.Example.ORG"), "app.example.org");
        assert_eq!(canonical("app.example.org."), "app.example.org");
        assert_eq!(canonical("localhost"), "localhost");

        // Internationalized names should use their ASCII form.
        assert_eq!(canonical("bücher.example.org"), "xn--bcher-kva.example.org");
        assert_eq!(
            canonical("BÜCHER.Example.org."),
            "xn--bcher-kva.example.org"
        );
        assert_eq!(
            canonical("XN--BCHER-KVA.example.org"),
            "xn--bcher-kva.example.org"
        );
        // Processing is non-transitional, the same as current browsers.
        assert_eq!(canonical("straße.example"), "xn--strae-oqa.example");
        // Other characters can map to label separators.
        assert_eq!(
            canonical("bücher。example。org"),
            "xn--bcher-kva.example.org"
        );

        // Names with empty labels or that cannot be converted are invalid.
        for name in [
            "",
            ".",
            "..",
            "docs.example..",
            "a..b",
            ".a",
            "a...",
            "docs。。example",
            "xn--a.example.org",
        ] {
            assert_eq!(canonical_name(name), None, "{name:?} should be invalid");
        }

        // Canonicalizing a canonical name should not change it.
        for name in ["App.Example.org.", "x.y", "Z", "Bücher.example.org"] {
            let once = canonical(name);
            assert_eq!(canonical(&once), once);
        }
    }

    #[test]
    fn test_handle_canonical_names() {
        let proxy = Proxy::new(Config::default(), acme_account()).unwrap();
        let handle = proxy.handle().clone();

        proxy.rt.block_on(async {
            for mode in [Mode::ForwardOnly, Mode::TerminateTls] {
                handle
                    .add_mapping(Mapping {
                        name: "App.Example.org".to_string(),
                        mode,
                        ..Default::default()
                    })
                    .await
                    .unwrap();
                let mapping = handle
                    .get_mapping("app.example.org")
                    .expect("mapping should exist");
                assert_eq!(mapping.name, "app.example.org");

                handle.remove_mapping("APP.example.org.").await;
                assert!(handle.get_mapping("app.example.org").is_none());

                // Internationalized names should be stored (and certificates provisioned) in
                // their ASCII form, which is what TLS clients send.
                handle
                    .add_mapping(Mapping {
                        name: "Bücher.example.org".to_string(),
                        mode,
                        ..Default::default()
                    })
                    .await
                    .unwrap();
                let mapping = handle
                    .get_mapping("xn--bcher-kva.example.org")
                    .expect("mapping should exist");
                assert_eq!(mapping.name, "xn--bcher-kva.example.org");
                handle.remove_mapping("xn--bcher-kva.example.org").await;
                assert!(handle.get_mapping("bücher.example.org").is_none());

                // A malformed name should not install a route that it cannot remove.
                let result = handle
                    .add_mapping(Mapping {
                        name: "docs.example..".to_string(),
                        mode,
                        ..Default::default()
                    })
                    .await;
                assert!(result.is_err());
                assert!(handle.get_mapping("docs.example").is_none());
            }
        });
    }
}
