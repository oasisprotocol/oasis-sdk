//! A proxy used by ROFL apps to easily handle and route HTTPS connections.
#![feature(once_cell_try)]

pub mod http;
pub mod wireguard;

/// Name of the label used to store the proxy configuration.
pub const LABEL_PROXY: &str = "net.oasis.proxy";
/// Domain separation context used for encrypting the proxy label.
pub const PROXY_LABEL_ENCRYPTION_CONTEXT: &str = ":rofl-proxy/label";

/// Value of the label used for app's proxy configuration.
///
/// NOTE: This label is usually encrypted using the app's SEK.
#[derive(Clone, Default, cbor::Encode, cbor::Decode)]
pub struct ProxyLabel {
    /// Wireguard client configuration.
    pub wireguard: wireguard::ClientConfig,
    /// HTTP proxy configuration.
    pub http: HttpConfig,
}

/// HTTP proxy configuration.
#[derive(Clone, Default, cbor::Encode, cbor::Decode)]
pub struct HttpConfig {
    /// Assigned HTTP host.
    pub host: String,
    /// Optional external IP address used by the proxy.
    pub external_address: Option<String>,
}
