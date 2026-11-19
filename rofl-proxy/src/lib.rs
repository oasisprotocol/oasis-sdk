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
    /// Whether the proxy sends a PROXY protocol header at the start of each connection.
    #[cbor(optional)]
    pub proxy_protocol: bool,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_http_config_proxy_protocol() {
        let mut cfg = HttpConfig {
            host: "example.com".to_string(),
            ..Default::default()
        };

        // Encoding should not change when the PROXY protocol is not used.
        let legacy = "a264686f73746b6578616d706c652e636f6d7065787465726e616c5f61646472657373f6";
        assert_eq!(hex::encode(cbor::to_vec(cfg.clone())), legacy);
        let dec: HttpConfig = cbor::from_slice(&hex::decode(legacy).unwrap()).unwrap();
        assert!(!dec.proxy_protocol);

        cfg.proxy_protocol = true;
        let dec: HttpConfig = cbor::from_slice(&cbor::to_vec(cfg)).unwrap();
        assert!(dec.proxy_protocol);
    }
}
