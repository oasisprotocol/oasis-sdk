mod pool;

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, Result};
use defguard_wireguard_rs::{
    host::Peer, key::Key, net::IpAddrMask, InterfaceConfiguration, Kernel, WGApi,
    WireguardInterfaceApi,
};
use oasis_runtime_sdk::core::common::crypto::x25519;

use pool::IpPool;

/// Name of the Wireguard interface.
const WG_INTERFACE_NAME: &str = "wg0";
/// Keep alive interval for Wireguard connections (in seconds).
const WG_KEEPALIVE_INTERVAL_SECS: u16 = 15;

/// Hub configuration.
pub struct HubConfig {
    pub external_address: String,
    pub external_port: u16,
}

struct HubState {
    pool: IpPool,
    clients: HashMap<x25519::PublicKey, IpAddr>,
}

/// A Wireguard hub that accepts connections from configured peers.
pub struct Hub {
    cfg: HubConfig,
    wg: WGApi<Kernel>,
    pk: x25519::PublicKey,
    address: IpAddrMask,
    state: Arc<Mutex<HubState>>,
}

impl Hub {
    /// Create a new instance of the Wireguard hub.
    pub fn new(cfg: HubConfig) -> Result<Self> {
        let wg = WGApi::new(WG_INTERFACE_NAME.to_string())?;
        wg.create_interface()?;

        let sk = x25519::PrivateKey::generate();
        let pk = sk.public_key();
        let sk: Key = sk.as_ref().try_into()?;

        let network: IpAddrMask = "100.64.0.0/10".parse().unwrap();
        let mut pool = IpPool::new(network.clone());
        let mut address = pool.allocate_host()?;
        address.cidr = network.cidr; // Avoid the need for individual routes.

        // TODO: Make sure forwarding between peers is not allowed.

        let if_cfg = InterfaceConfiguration {
            name: WG_INTERFACE_NAME.to_string(),
            prvkey: sk.to_lower_hex(),
            addresses: vec![address.clone()],
            port: cfg.external_port.into(),
            peers: vec![],
            mtu: None,
        };
        wg.configure_interface(&if_cfg)?;

        Ok(Self {
            cfg,
            wg,
            pk,
            address,
            state: Arc::new(Mutex::new(HubState {
                pool,
                clients: HashMap::new(),
            })),
        })
    }

    /// Provision a new client key pair, assign addresses and return its config.
    pub fn provision_client(&self) -> Result<ClientConfig> {
        let mut state = self.state.lock().unwrap();
        let mut host = state.pool.allocate_host()?;
        host.cidr = self.address.cidr;

        let sk = x25519::PrivateKey::generate();
        let pk = sk.public_key();

        let mut peer = Peer::new(pk.as_ref().try_into()?);
        peer.allowed_ips = vec![host.clone()];
        peer.persistent_keepalive_interval = Some(WG_KEEPALIVE_INTERVAL_SECS);

        if let Err(err) = self.wg.configure_peer(&peer) {
            state.pool.return_allocated_host(host.ip);
            return Err(err.into());
        }

        state.clients.insert(pk, host.ip);

        Ok(ClientConfig {
            sk,
            address: host.to_string(),
            hub_pk: self.pk,
            hub_endpoint: format!("{}:{}", self.cfg.external_address, self.cfg.external_port),
            hub_address: self.address.to_string(),
        })
    }

    /// Deprovision a given client key.
    pub fn deprovision_client(&self, pk: &x25519::PublicKey) -> Result<()> {
        let mut state = self.state.lock().unwrap();

        self.wg.remove_peer(&pk.as_ref().try_into()?)?;
        let client_address = state
            .clients
            .remove(pk)
            .ok_or(anyhow!("client not found"))?;
        state.pool.return_allocated_host(client_address);

        Ok(())
    }
}

/// Client configuration.
#[derive(Clone, Default, cbor::Encode, cbor::Decode)]
pub struct ClientConfig {
    /// Client secret key.
    pub sk: x25519::PrivateKey,
    /// Assigned client address.
    pub address: String,

    /// Hub public key.
    pub hub_pk: x25519::PublicKey,
    /// Hub external endpoint address.
    pub hub_endpoint: String,
    /// Hub internal address.
    pub hub_address: String,
}

/// A Wireguard client that connects to the hub.
pub struct Client {
    _wg: WGApi<Kernel>,
}

impl Client {
    /// Create a new Wireguard client.
    pub fn new(cfg: ClientConfig) -> Result<Self> {
        let wg = WGApi::new(WG_INTERFACE_NAME.to_string())?;
        wg.create_interface()?;

        let mut peer_hub = Peer::new(cfg.hub_pk.as_ref().try_into()?);
        peer_hub.endpoint = Some(cfg.hub_endpoint.parse()?);
        peer_hub.allowed_ips = vec![cfg.hub_address.parse()?];
        peer_hub.persistent_keepalive_interval = Some(WG_KEEPALIVE_INTERVAL_SECS);

        let sk: Key = cfg.sk.as_ref().try_into()?;

        let cfg = InterfaceConfiguration {
            name: WG_INTERFACE_NAME.to_string(),
            prvkey: sk.to_lower_hex(),
            addresses: vec![cfg.address.parse()?],
            port: 4040,
            peers: vec![peer_hub],
            mtu: None,
        };
        wg.configure_interface(&cfg)?;

        Ok(Self { _wg: wg })
    }
}
