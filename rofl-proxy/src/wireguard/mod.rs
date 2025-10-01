mod pool;

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, Result};
use defguard_wireguard_rs::{
    host::{self, Peer},
    key::Key,
    net::IpAddrMask,
    InterfaceConfiguration, Kernel, WGApi, WireguardInterfaceApi,
};
use oasis_runtime_sdk::core::common::crypto::x25519;

use pool::IpPool;

/// Name of the Wireguard interface.
pub const WG_INTERFACE_NAME: &str = "wg0";
/// Keep alive interval for Wireguard connections (in seconds).
pub const WG_KEEPALIVE_INTERVAL_SECS: u16 = 15;
/// Network to use for the Wireguard interface.
pub const WG_NETWORK: &str = "100.64.0.0/10";
/// Default Wireguard listen port.
pub const WG_DEFAULT_LISTEN_PORT: u16 = 4040;

/// Hub configuration.
#[derive(Clone)]
pub struct HubConfig {
    /// External IP address of the hub.
    pub external_address: String,
    /// External UDP port of the hub.
    pub external_port: u16,
}

struct HubState {
    pool: IpPool,
    clients: HashMap<x25519::PublicKey, IpAddr>,
}

/// A Wireguard hub that accepts connections from configured peers.
#[derive(Clone)]
pub struct Hub {
    cfg: HubConfig,
    wg: Arc<WGApi<Kernel>>,
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

        let network: IpAddrMask = WG_NETWORK.parse().unwrap();
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
            wg: Arc::new(wg),
            pk,
            address,
            state: Arc::new(Mutex::new(HubState {
                pool,
                clients: HashMap::new(),
            })),
        })
    }

    /// Returns the current status of the WireGuard interface.
    pub fn current_status(&self) -> Result<host::Host> {
        Ok(self.wg.read_interface_data()?)
    }

    /// Provision a new client key pair, assign addresses and return its config.
    pub fn provision_client(&self) -> Result<ClientConfig> {
        let mut state = self.state.lock().unwrap();
        let mut host = state.pool.allocate_host()?;

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

        // Update CIDR in client configuration so the client doesn't need extra routes.
        host.cidr = self.address.cidr;

        Ok(ClientConfig {
            sk,
            address: host.to_string(),
            listen_port: WG_DEFAULT_LISTEN_PORT,
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
    /// Listen port.
    pub listen_port: u16,

    /// Hub public key.
    pub hub_pk: x25519::PublicKey,
    /// Hub external endpoint address.
    pub hub_endpoint: String,
    /// Hub internal address.
    pub hub_address: String,
}

/// A Wireguard client that connects to the hub.
pub struct Client {
    cfg: Option<ClientConfig>,
    wg: WGApi<Kernel>,
}

impl Client {
    /// Create a new Wireguard client.
    pub fn new(cfg: ClientConfig) -> Result<Self> {
        let wg = WGApi::new(WG_INTERFACE_NAME.to_string())?;
        Ok(Self { cfg: Some(cfg), wg })
    }

    /// Start the Wireguard client by creating and configuring the necessary interfaces.
    pub fn start(&mut self) -> Result<()> {
        let cfg = match self.cfg.take() {
            Some(cfg) => cfg,
            None => return Ok(()),
        };

        self.wg.create_interface()?;

        let mut peer_hub = Peer::new(cfg.hub_pk.as_ref().try_into()?);
        peer_hub.endpoint = Some(cfg.hub_endpoint.parse()?);
        peer_hub.allowed_ips = vec![cfg.hub_address.parse()?];
        peer_hub.persistent_keepalive_interval = Some(WG_KEEPALIVE_INTERVAL_SECS);

        let sk: Key = cfg.sk.as_ref().try_into()?;

        let cfg = InterfaceConfiguration {
            name: WG_INTERFACE_NAME.to_string(),
            prvkey: sk.to_lower_hex(),
            addresses: vec![cfg.address.parse()?],
            port: cfg.listen_port.into(),
            peers: vec![peer_hub],
            mtu: None,
        };
        self.wg.configure_interface(&cfg)?;

        Ok(())
    }
}
