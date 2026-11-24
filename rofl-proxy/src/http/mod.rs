mod idle;
pub mod proxy;
pub mod proxy_protocol;
pub mod sni;
pub mod tls;

pub use proxy::{
    canonical_name, Config, ConnectionGroup, Mapping, Mode, Proxy, ProxyHandle, Timeouts,
    MAX_TIMEOUT,
};
