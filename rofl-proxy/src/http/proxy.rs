use std::{
    io::Cursor,
    net::{IpAddr, SocketAddr},
    pin::pin,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use oasis_runtime_sdk::core::common::logger::get_logger;
use ptrie::Trie;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::{TcpListener, TcpStream},
    sync::Semaphore,
    time::timeout,
};
use tokio_io_timeout::TimeoutStream;
use tokio_rustls::TlsAcceptor;

use crate::http::tls;

use super::{
    proxy_protocol, sni,
    tls::{CertificateProvisioner, CertificateProvisionerHandle, ACME_TLS_ALPN_PROTOCOL_ID},
};

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

/// Proxy configuration.
pub struct Config {
    pub listen_address: String,
    pub listen_port: u16,

    pub timeout_handshake: Duration,
    pub timeout_connect: Duration,
    pub timeout_connection: Duration,
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

/// Proxy mapping.
#[derive(Clone, Default, Debug)]
pub struct Mapping {
    /// Domain name to map (based on SNI extension).
    pub name: String,
    /// Destination address to connect to.
    pub dst_address: String,
    /// Destination port to connect to.
    pub dst_port: u16,
    /// Proxy mode for this mapping.
    pub mode: Mode,
    /// Whether to send a PROXY protocol header to the destination.
    pub proxy_protocol: bool,
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

    /// Add a new proxy mapping.
    ///
    /// If a mapping for the same domain exists, it is overwritten.
    fn add(&self, mapping: Mapping) {
        let mut tree = self.tree.lock().unwrap();
        tree.insert(reverse(&mapping.name.clone()), Arc::new(mapping));
    }

    /// Remove an existing proxy mapping, returning it.
    fn remove(&self, name: &str) -> Option<Arc<Mapping>> {
        let mut tree = self.tree.lock().unwrap();
        tree.remove(reverse(name))
    }

    /// Lookup an existing proxy mapping by using the longest suffix match.
    fn get(&self, name: &str) -> Option<Arc<Mapping>> {
        let tree = self.tree.lock().unwrap();
        tree.find_longest_prefix(reverse(name)).cloned()
    }
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
    pub async fn add_mapping(&self, mapping: Mapping) {
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

                        if let Err(err) =
                            Self::handle_connection_with_timeout(state, stream, peer).await
                        {
                            slog::error!(logger, "failed to handle connection"; "err" => ?err);
                        }
                        drop(connection_permit);
                    });
                }
                Err(_) => continue,
            }
        }
    }

    async fn handle_connection_with_timeout(
        state: Arc<State>,
        stream: TcpStream,
        peer: SocketAddr,
    ) -> Result<()> {
        timeout(
            state.cfg.timeout_connection,
            Self::handle_connection(state, stream, peer),
        )
        .await
        .context("max connection time limit reached")?
    }

    async fn handle_connection(
        state: Arc<State>,
        mut stream: TcpStream,
        peer: SocketAddr,
    ) -> Result<()> {
        let (proxy_addrs, tls_hello) = timeout(state.cfg.timeout_handshake, async {
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

        // Prepare the PROXY protocol header for the destination, forwarding the addresses received
        // from the hub when available.
        let proxy_header = match (mapping.proxy_protocol, proxy_addrs) {
            (false, _) => None,
            (true, Some((src, dst))) => Some(proxy_protocol::encode(src, dst)),
            (true, None) => Some(proxy_protocol::encode(peer, stream.local_addr()?)),
        };

        let connect_to_destination = async || -> Result<_> {
            let dst = timeout(
                state.cfg.timeout_connect,
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
                Self::handle_forwarding(state, &mut stream, &mut dst).await?;
            }
            Mode::TerminateTls => {
                // Handle TLS termination.
                let mut stream = state
                    .acceptor
                    .accept_with(stream, |conn| {
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
                    })
                    .await?;

                // Skip further connection processing in case of ACME-TLS ALPN protocol.
                if stream.get_ref().1.alpn_protocol() == Some(ACME_TLS_ALPN_PROTOCOL_ID) {
                    return Ok(());
                }

                let mut dst = connect_to_destination().await?;
                if let Some(header) = proxy_header {
                    dst.write_all(&header).await?;
                }
                Self::handle_forwarding(state, &mut stream, &mut dst).await?;
            }
        }

        Ok(())
    }

    async fn handle_forwarding<A, B>(state: Arc<State>, a: &mut A, b: &mut B) -> Result<()>
    where
        A: AsyncRead + AsyncWrite + Unpin,
        B: AsyncRead + AsyncWrite + Unpin,
    {
        // Impose read/write timeouts.
        let mut a = TimeoutStream::new(a);
        a.set_read_timeout(Some(state.cfg.timeout_rw));
        a.set_write_timeout(Some(state.cfg.timeout_rw));
        let mut a = pin!(a);

        let mut b = TimeoutStream::new(b);
        b.set_read_timeout(Some(state.cfg.timeout_rw));
        b.set_write_timeout(Some(state.cfg.timeout_rw));
        let mut b = pin!(b);

        tokio::io::copy_bidirectional(&mut a, &mut b).await?;

        Ok(())
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
    pub async fn add_mapping(&self, mapping: Mapping) {
        if matches!(mapping.mode, Mode::TerminateTls) {
            self.state.provisioner.add_domain(&mapping.name).await;
        }
        self.state.mappings.add(mapping);
    }

    /// Remove an existing proxy mapping.
    pub async fn remove_mapping(&self, name: &str) {
        let mapping = self.state.mappings.remove(name);
        if let Some(mapping) = mapping {
            if matches!(mapping.mode, Mode::TerminateTls) {
                self.state.provisioner.remove_domain(name).await;
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
            proxy.state.mappings.add(Mapping {
                name: "example.net".to_string(),
                dst_address: "127.0.0.1".to_string(),
                dst_port: destination.local_addr()?.port(),
                mode: Mode::ForwardOnly,
                proxy_protocol,
            });

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

        mappings.add(Mapping {
            name: "foo.example.com".to_string(),
            dst_address: "a".to_string(),
            dst_port: 1234,
            mode: Mode::ForwardOnly,
            proxy_protocol: false,
        });
        mappings.add(Mapping {
            name: "bar.foo.example.com".to_string(),
            dst_address: "b".to_string(),
            dst_port: 1234,
            mode: Mode::ForwardOnly,
            proxy_protocol: false,
        });
        mappings.add(Mapping {
            name: "another.example.com".to_string(),
            dst_address: "c".to_string(),
            dst_port: 1234,
            mode: Mode::ForwardOnly,
            proxy_protocol: false,
        });

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
    }
}
