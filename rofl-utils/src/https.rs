//! A very simple HTTPS client that can be used inside ROFL apps.
//!
//! This simple client is needed because Fortanix EDP does not yet have support for mio/Tokio
//! networking and so the usual `hyper` and `reqwest` cannot be used without patches.
use std::{
    fmt,
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream},
    sync::{Arc, OnceLock},
};

use rustls::{ClientConfig, ClientConnection, StreamOwned};
use rustls_pki_types::ServerName;
use ureq::{
    http::Uri,
    resolver::Resolver,
    transport::{
        time::NextTimeout, Buffers, ChainedConnector, ConnectionDetails, Connector, LazyBuffers,
        Transport, TransportAdapter,
    },
    Agent, AgentConfig,
};

/// An `ureq::Agent` that can be used to perform blocking HTTPS requests.
///
/// Note that this forbids non-HTTPS requests. If you need to perform plain HTTP requests consider
/// using `agent_with_config` and pass a suitable config.
pub fn agent() -> Agent {
    let cfg = AgentConfig {
        https_only: true, // Not using HTTPS is unsafe unless careful.
        user_agent: "rofl-utils/0.1.0".to_string(),
        ..Default::default()
    };
    agent_with_config(cfg)
}

/// An `ureq::Agent` with given configuration that can be used to perform blocking HTTPS requests.
pub fn agent_with_config(cfg: AgentConfig) -> Agent {
    Agent::with_parts(
        cfg,
        ChainedConnector::new([SgxConnector.boxed(), RustlsConnector::default().boxed()]),
        SgxResolver,
    )
}

#[derive(Debug)]
struct SgxConnector;

impl Connector for SgxConnector {
    fn connect(
        &self,
        details: &ConnectionDetails,
        _chained: Option<Box<dyn Transport>>,
    ) -> Result<Option<Box<dyn Transport>>, ureq::Error> {
        let config = &details.config;
        // Has already been validated.
        let scheme = details.uri.scheme().unwrap();
        let authority = details.uri.authority().unwrap();

        let host_port = ureq::resolver::DefaultResolver::host_and_port(scheme, authority)
            .ok_or(ureq::Error::HostNotFound)?;
        let stream = TcpStream::connect(host_port)?;

        let buffers = LazyBuffers::new(config.input_buffer_size, config.output_buffer_size);
        let transport = TcpTransport::new(stream, buffers);

        Ok(Some(Box::new(transport)))
    }
}

struct TcpTransport {
    stream: TcpStream,
    buffers: LazyBuffers,
}

impl TcpTransport {
    fn new(stream: TcpStream, buffers: LazyBuffers) -> TcpTransport {
        TcpTransport { stream, buffers }
    }
}

impl Transport for TcpTransport {
    fn buffers(&mut self) -> &mut dyn Buffers {
        &mut self.buffers
    }

    fn transmit_output(&mut self, amount: usize, _timeout: NextTimeout) -> Result<(), ureq::Error> {
        let output = &self.buffers.output()[..amount];
        self.stream.write_all(output)?;

        Ok(())
    }

    fn await_input(&mut self, _timeout: NextTimeout) -> Result<bool, ureq::Error> {
        if self.buffers.can_use_input() {
            return Ok(true);
        }

        let input = self.buffers.input_mut();
        let amount = self.stream.read(input)?;
        self.buffers.add_filled(amount);

        Ok(amount > 0)
    }

    fn is_open(&mut self) -> bool {
        // No way to detect on SGX.
        true
    }
}

impl fmt::Debug for TcpTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TcpTransport")
            .field("addr", &self.stream.peer_addr().ok())
            .finish()
    }
}

#[derive(Debug)]
struct SgxResolver;

impl Resolver for SgxResolver {
    fn resolve(
        &self,
        _uri: &Uri,
        _config: &AgentConfig,
        _timeout: NextTimeout,
    ) -> Result<ureq::resolver::ResolvedSocketAddrs, ureq::Error> {
        // Do not resolve anything as SGX does not support resolution and the endpoint must be
        // passed as a string. We need to return a dummy address.
        Ok(vec![SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::new(0, 0, 0, 0),
            0,
        ))]
        .into())
    }
}

#[derive(Default)]
struct RustlsConnector {
    config: OnceLock<Arc<ClientConfig>>,
}

impl Connector for RustlsConnector {
    fn connect(
        &self,
        details: &ConnectionDetails,
        chained: Option<Box<dyn Transport>>,
    ) -> Result<Option<Box<dyn Transport>>, ureq::Error> {
        let Some(transport) = chained else {
            panic!("RustlsConnector requires a chained transport");
        };

        // Only add TLS if we are connecting via HTTPS and the transport isn't TLS
        // already, otherwise use chained transport as is.
        if !details.needs_tls() || transport.is_tls() {
            return Ok(Some(transport));
        }

        // Initialize the config on first run.
        let config_ref = self.config.get_or_init(build_config);
        let config = config_ref.clone();

        let name_borrowed: ServerName<'_> = details
            .uri
            .authority()
            .ok_or(ureq::Error::HostNotFound)?
            .host()
            .try_into()
            .map_err(|_| ureq::Error::HostNotFound)?;

        let name = name_borrowed.to_owned();

        let conn =
            ClientConnection::new(config, name).map_err(|_| ureq::Error::ConnectionFailed)?;
        let stream = StreamOwned {
            conn,
            sock: TransportAdapter::new(transport),
        };

        let buffers = LazyBuffers::new(
            details.config.input_buffer_size,
            details.config.output_buffer_size,
        );

        let transport = Box::new(RustlsTransport { buffers, stream });

        Ok(Some(transport))
    }
}

fn build_config() -> Arc<ClientConfig> {
    let provider = Arc::new(rustls_mbedcrypto_provider::mbedtls_crypto_provider());

    let builder = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap();

    let builder = builder
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(
            rustls_mbedpki_provider::MbedTlsServerCertVerifier::new(
                webpki_root_certs::TLS_SERVER_ROOT_CERTS,
            )
            .unwrap(),
        ));

    let config = builder.with_no_client_auth();

    Arc::new(config)
}

struct RustlsTransport {
    buffers: LazyBuffers,
    stream: StreamOwned<ClientConnection, TransportAdapter>,
}

impl Transport for RustlsTransport {
    fn buffers(&mut self) -> &mut dyn Buffers {
        &mut self.buffers
    }

    fn transmit_output(&mut self, amount: usize, _timeout: NextTimeout) -> Result<(), ureq::Error> {
        let output = &self.buffers.output()[..amount];
        self.stream.write_all(output)?;

        Ok(())
    }

    fn await_input(&mut self, _timeout: NextTimeout) -> Result<bool, ureq::Error> {
        if self.buffers.can_use_input() {
            return Ok(true);
        }

        let input = self.buffers.input_mut();
        let amount = self.stream.read(input)?;
        self.buffers.add_filled(amount);

        Ok(amount > 0)
    }

    fn is_open(&mut self) -> bool {
        self.stream.get_mut().get_mut().is_open()
    }

    fn is_tls(&self) -> bool {
        true
    }
}

impl fmt::Debug for RustlsConnector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RustlsConnector").finish()
    }
}

impl fmt::Debug for RustlsTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RustlsTransport").finish()
    }
}

#[cfg(test)]
mod test {
    use mockito::{mock, server_url};

    use super::{agent, agent_with_config};

    #[test]
    fn test_get_request() {
        // Set up a mock server
        let _mock = mock("GET", "/test")
            .with_status(200)
            .with_header("content-type", "text/plain")
            .with_body("Hello, world!")
            .create();

        // Create an agent
        let agent = agent_with_config(Default::default());

        // Make a GET request to the mock server
        let url = format!("{}/test", server_url());
        let mut response = agent.get(&url).call().unwrap();

        // Verify the response
        assert_eq!(response.status(), 200);
        assert_eq!(
            response.body_mut().read_to_string().unwrap(),
            "Hello, world!"
        );
    }

    #[test]
    fn test_post_request() {
        // Set up a mock server for POST request
        let _mock = mock("POST", "/submit")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{"success":true}"#)
            .create();

        // Create an agent
        let agent = agent_with_config(Default::default());

        // Make a POST request to the mock server
        let url = format!("{}/submit", server_url());
        let mut response = agent
            .post(&url)
            .content_type("application/json")
            .send(r#"{"key":"value"}"#)
            .unwrap();

        // Verify the response
        assert_eq!(response.status(), 201);
        assert_eq!(
            response.body_mut().read_to_string().unwrap(),
            r#"{"success":true}"#
        );
    }

    #[test]
    fn test_get_remote_https() {
        let response = agent().get("https://www.google.com/").call().unwrap();

        // Verify the response
        assert_eq!(
            "text/html;charset=ISO-8859-1",
            response
                .headers()
                .get("content-type")
                .unwrap()
                .to_str()
                .unwrap()
                .replace("; ", ";")
        );
        assert_eq!(response.body().mime_type(), Some("text/html"));
    }
}
