use crate::types::token;

/// Module configuration.
pub trait Config: 'static {
    /// Gas cost of roflmarket.ProviderCreate call.
    const GAS_COST_CALL_PROVIDER_CREATE: u64 = 100_000;
    const GAS_COST_CALL_INSTANCE_CREATE: u64 = 100_000;
    const GAS_COST_CALL_INSTANCE_ACCEPT: u64 = 10_000;
    const GAS_COST_CALL_INSTANCE_CANCEL: u64 = 10_000;
    const GAS_COST_CALL_INSTANCE_EXECUTE: u64 = 50_000;
    const GAS_COST_CALL_INSTANCE_CLEAR_COMMANDS_BASE: u64 = 10_000;
    const GAS_COST_CALL_INSTANCE_CLEAR_COMMANDS_CMD_BASE: u64 = 10_000;
    const GAS_COST_CALL_INSTANCE_CLEAR_COMMANDS_CMD_DEPLOY: u64 = 40_000;

    /// Maximum number of queued instance commands.
    const MAX_QUEUED_INSTANCE_COMMANDS: u64 = 16;

    /// Amount of stake required for maintaining a provider.
    ///
    /// The stake is held in escrow and is returned to the provider when the entry is removed.
    const STAKE_PROVIDER_CREATE: token::BaseUnits =
        token::BaseUnits::new(0, token::Denomination::NATIVE);
}
