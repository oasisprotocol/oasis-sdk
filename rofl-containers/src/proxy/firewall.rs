use anyhow::Result;
use rustables::{
    expr::{Cmp, CmpOp, Meta, MetaType},
    Batch, Chain, ChainPolicy, Hook, HookClass, MsgType, Protocol, ProtocolFamily, Rule, Table,
};

/// Name of the netfilter table.
const TABLE_NAME: &str = "rofl-proxy";
/// Name of the Wireguard chain.
const WG_CHAIN_NAME: &str = "wireguard";

/// Firewall.
pub struct Firewall {
    batch: Batch,
    table: Table,
}

impl Firewall {
    /// Create a new firewall.
    pub fn new() -> Self {
        let mut batch = Batch::new();

        // Create the table for our firewall.
        let table = Table::new(ProtocolFamily::Inet).with_name(TABLE_NAME);
        batch.add(&table, MsgType::Add);

        Self { batch, table }
    }

    /// Add Wireguard chain.
    pub fn add_wireguard(
        &mut self,
        iface: &str,
        hub_address: &str,
        proxy_address: &str,
        proxy_port: u16,
    ) -> Result<()> {
        let chain_wg = Chain::new(&self.table)
            .with_name(WG_CHAIN_NAME)
            .with_hook(Hook::new(HookClass::In, 0))
            .with_policy(ChainPolicy::Drop)
            .add_to_batch(&mut self.batch);

        // Only process traffic coming from the wireguard interface, accept everything else.
        let mut iface = iface.as_bytes().to_vec();
        iface.push(0u8);

        Rule::new(&chain_wg)?
            .with_expr(Meta::new(MetaType::IifName))
            .with_expr(Cmp::new(CmpOp::Neq, iface))
            .accept()
            .add_to_batch(&mut self.batch);

        // Accept traffic going to the proxy from the hub.
        Rule::new(&chain_wg)?
            .saddr(hub_address.parse()?)
            .daddr(proxy_address.parse()?)
            .dport(proxy_port, Protocol::TCP)
            .accept()
            .add_to_batch(&mut self.batch);

        // Drop everything else.
        Ok(())
    }

    /// Start the firewall by committing the netfilter table.
    pub fn start(self) -> Result<()> {
        self.batch.send()?;
        Ok(())
    }
}
