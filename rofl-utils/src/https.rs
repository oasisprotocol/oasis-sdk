//! A very simple HTTPS client that can be used inside ROFL apps.
//!
//! This simple client is needed because Fortanix EDP does not yet have support for mio/Tokio
//! networking and so the usual `hyper` and `reqwest` cannot be used without patches.
use std::{
    fmt,
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream},
};

use ureq::{
    http::Uri,
    resolver::Resolver,
    tls::RustlsConnector,
    transport::{
        time::NextTimeout, Buffers, ChainedConnector, ConnectionDetails, Connector, LazyBuffers,
        Transport,
    },
    Agent, AgentConfig,
};

/// An `ureq::Agent` that can be used to perform blocking HTTPS requests.
pub fn agent() -> Agent {
    // Production configuration.
    #[cfg(not(test))]
    let cfg = AgentConfig {
        https_only: true, // Not using HTTPS is unsafe.
        ..Default::default()
    };

    // Test configuration.
    #[cfg(test)]
    let cfg = AgentConfig {
        https_only: false,
        ..Default::default()
    };

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

#[cfg(test)]
mod test {
    use mockito::{mock, server_url};

    use super::agent;

    #[test]
    fn test_get_request() {
        // Set up a mock server
        let _mock = mock("GET", "/test")
            .with_status(200)
            .with_header("content-type", "text/plain")
            .with_body("Hello, world!")
            .create();

        // Create an agent
        let agent = agent();

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
        let agent = agent();

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
}
