use crate::modules;

/// Incoming message handler configuration.
pub trait Config: 'static {
    /// The accounts module to use.
    type Accounts: modules::accounts::API;
    /// The consensus module to use.
    type Consensus: modules::consensus::API;

    /// Maximum number of outgoing consensus message slots that an incoming message can claim.
    ///
    /// When this is configured to be greater than zero it allows incoming messages to also emit
    /// consensus messages as a result of executing a transaction.
    const MAX_CONSENSUS_MSG_SLOTS_PER_TX: u32 = 1;
}
