use std::{
    io::Cursor,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use oasis_runtime_sdk::core::common::logger::get_logger;
use ptrie::Trie;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::{TcpListener, TcpStream},
    time::timeout,
};
use tokio_rustls::TlsAcceptor;

use super::{
    sni,
    tls::{CertificateProvisioner, CertificateProvisionerHandle, ACME_TLS_ALPN_PROTOCOL_ID},
};

/// Proxy operation mode.
#[derive(Debug, Clone, Copy)]
pub enum Mode {
    /// Only forward encrypted TLS records based on initial SNI information.
    ForwardOnly,
    /// Terminate TLS, provision certificates via ACME for all mappings and forward based on
    /// initial SNI information.
    TerminateTls,
}

/// Proxy configuration.
pub struct Config {
    pub mode: Mode,
    pub listen_address: String,
    pub listen_port: u16,

    pub timeout_handshake: Duration,
    pub timeout_connect: Duration,
    pub timeout_connection: Duration,
    pub timeout_rw: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: Mode::ForwardOnly,
            listen_address: "0.0.0.0".to_string(),
            listen_port: 443,
            timeout_handshake: Duration::from_secs(1),
            timeout_connect: Duration::from_secs(1),
            timeout_connection: Duration::from_secs(30),
            timeout_rw: Duration::from_secs(5),
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
    fn add(&self, mapping: Mapping) {
        let mut tree = self.tree.lock().unwrap();
        tree.insert(
            mapping.name.clone().split('.').rev().map(|s| s.to_string()),
            Arc::new(mapping),
        );
    }

    /// Remove an existing proxy mapping.
    fn remove(&self, name: &str) {
        let mut tree = self.tree.lock().unwrap();
        tree.remove(name.split('.').rev().map(|s| s.to_string()));
    }

    /// Lookup an existing proxy mapping.
    fn get(&self, name: &str) -> Option<Arc<Mapping>> {
        let tree = self.tree.lock().unwrap();
        tree.find_longest_prefix(name.split('.').rev().map(|s| s.to_string()))
            .cloned()
    }
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
    provisioner: Option<CertificateProvisioner>,
}

impl Proxy {
    /// Create a new proxy instance.
    pub fn new(cfg: Config) -> Result<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .thread_name("rofl-proxy")
            .enable_all()
            .build()?;

        let provisioner = CertificateProvisioner::new();
        let state = State {
            cfg,
            mappings: Mappings::new(),
            provisioner: provisioner.handle().clone(),
            acceptor: TlsAcceptor::from(provisioner.server_config(false)),
        };

        Ok(Self {
            rt,
            state: Arc::new(state),
            provisioner: Some(provisioner),
        })
    }

    /// Add a new proxy mapping.
    pub async fn add_mapping(&self, mapping: Mapping) {
        if matches!(self.state.cfg.mode, Mode::TerminateTls) {
            self.state.provisioner.add_domain(&mapping.name).await;
        }
        self.state.mappings.add(mapping);
    }

    /// Remove an existing proxy mapping.
    pub async fn remove_mapping(&self, name: &str) {
        self.state.mappings.remove(name);
    }

    /// Lookup an existing proxy mapping.
    pub fn get_mapping(&self, name: &str) -> Option<Arc<Mapping>> {
        self.state.mappings.get(name)
    }

    /// Start the proxy.
    pub fn start(&mut self) {
        match (self.provisioner.take(), self.state.cfg.mode) {
            (Some(provisioner), Mode::TerminateTls) => {
                self.rt.spawn(async move { provisioner.start() });
            }
            (Some(_), _) => {}
            (None, _) => return,
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
        let logger = get_logger("proxy/http");
        let listener =
            TcpListener::bind((state.cfg.listen_address.clone(), state.cfg.listen_port)).await?;

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let logger = logger.clone();
                    let state = state.clone();

                    tokio::spawn(async move {
                        if let Err(err) = Self::handle_connection_with_timeout(state, stream).await
                        {
                            slog::info!(logger, "failed to handle connection"; "err" => ?err);
                        }
                    });
                }
                Err(err) => {
                    slog::warn!(logger, "failed to accept connection"; "err" => ?err);
                }
            }
        }
    }

    async fn handle_connection_with_timeout(state: Arc<State>, stream: TcpStream) -> Result<()> {
        timeout(
            state.cfg.timeout_connection,
            Self::handle_connection(state, stream),
        )
        .await
        .context("max connection time limit reached")?
    }

    async fn handle_connection(state: Arc<State>, mut stream: TcpStream) -> Result<()> {
        let tls_hello = timeout(
            state.cfg.timeout_handshake,
            Self::parse_tls_hello(&mut stream),
        )
        .await
        .context("TLS handshake timeout")?
        .context("failed to parse TLS hello")?;

        // Resolve destination.
        let mapping = state
            .mappings
            .get(&tls_hello.sni)
            .ok_or(anyhow!("unknown host"))?;

        let connect_to_destination = async || -> Result<_> {
            let dst = timeout(
                state.cfg.timeout_connect,
                TcpStream::connect((mapping.dst_address.clone(), mapping.dst_port)),
            )
            .await
            .context("connect to destination timeout")?
            .context("failed to connect to destination")?;
            Ok(dst)
        };

        match state.cfg.mode {
            Mode::ForwardOnly => {
                // Replay TLS handshake buffer and forward.
                let mut dst = connect_to_destination().await?;
                dst.write_all(&tls_hello.raw).await?;
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
                Self::handle_forwarding(state, &mut stream, &mut dst).await?;
            }
        }

        Ok(())
    }

    async fn handle_forwarding<A, B>(_state: Arc<State>, a: &mut A, b: &mut B) -> Result<()>
    where
        A: AsyncRead + AsyncWrite + Unpin,
        B: AsyncRead + AsyncWrite + Unpin,
    {
        // TODO: Impose read/write timeouts.
        tokio::io::copy_bidirectional(a, b).await?;

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

/// Information extracted from a TLS ClientHello message.
struct TlsHello {
    /// Extracted value of the SNI extension.
    sni: String,
    /// Raw data read to parse the TLS record.
    raw: Vec<u8>,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_mappings() {
        let mappings = Mappings::new();
        let mapping = mappings.get("example.com");
        assert!(mapping.is_none());

        mappings.add(Mapping {
            name: "foo.example.com".to_string(),
            dst_address: "a".to_string(),
            dst_port: 1234,
        });
        mappings.add(Mapping {
            name: "bar.foo.example.com".to_string(),
            dst_address: "b".to_string(),
            dst_port: 1234,
        });
        mappings.add(Mapping {
            name: "another.example.com".to_string(),
            dst_address: "c".to_string(),
            dst_port: 1234,
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
