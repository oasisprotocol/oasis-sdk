use oasis_runtime_sdk::{modules, types::token};

/// Module configuration.
pub trait Config: 'static {
    /// Module implementing the ROFL API.
    type Rofl: modules::rofl::API;

    /// Gas cost of roflmarket.ProviderCreate call.
    const GAS_COST_CALL_PROVIDER_CREATE: u64 = 100_000;
    const GAS_COST_CALL_PROVIDER_UPDATE: u64 = 100_000;
    const GAS_COST_CALL_PROVIDER_UPDATE_OFFERS_BASE: u64 = 100_000;
    const GAS_COST_CALL_PROVIDER_UPDATE_OFFERS_ADD: u64 = 10_000;
    const GAS_COST_CALL_PROVIDER_UPDATE_OFFERS_RM: u64 = 1_000;
    const GAS_COST_CALL_PROVIDER_REMOVE: u64 = 100_000;
    const GAS_COST_CALL_INSTANCE_CREATE: u64 = 100_000;
    const GAS_COST_CALL_INSTANCE_ACCEPT_BASE: u64 = 10_000;
    const GAS_COST_CALL_INSTANCE_ACCEPT_INSTANCE: u64 = 10_000;
    const GAS_COST_CALL_INSTANCE_UPDATE: u64 = 100_000;
    const GAS_COST_CALL_INSTANCE_CANCEL: u64 = 10_000;
    const GAS_COST_CALL_INSTANCE_REMOVE: u64 = 10_000;
    const GAS_COST_CALL_INSTANCE_EXECUTE_CMDS_BASE: u64 = 10_000;
    const GAS_COST_CALL_INSTANCE_EXECUTE_CMDS_CMD: u64 = 10_000;
    const GAS_COST_CALL_INSTANCE_COMPLETE_CMDS_BASE: u64 = 10_000;
    const GAS_COST_CALL_INSTANCE_COMPLETE_CMDS_CMD: u64 = 10_000;

    /// Maximum number of offers a provider can have.
    const MAX_PROVIDER_OFFERS: u64 = 64;
    /// Maximum number of queued instance commands.
    const MAX_QUEUED_INSTANCE_COMMANDS: u64 = 16;

    /// Amount of stake required for maintaining a provider.
    ///
    /// The stake is held in escrow and is returned to the provider when the entry is removed.
    const STAKE_PROVIDER_CREATE: token::BaseUnits =
        token::BaseUnits::new(0, token::Denomination::NATIVE);
}
