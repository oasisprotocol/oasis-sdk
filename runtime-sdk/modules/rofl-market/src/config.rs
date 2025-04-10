use oasis_runtime_sdk::{modules, types::token};

/// Module configuration.
pub trait Config: 'static {
    /// Module implementing the ROFL API.
    type Rofl: modules::rofl::API;

    /// Gas cost of roflmarket.ProviderCreate call.
    const GAS_COST_CALL_PROVIDER_CREATE: u64 = 100_000;
    /// Gas cost of roflmarket.ProviderUpdate call.
    const GAS_COST_CALL_PROVIDER_UPDATE: u64 = 100_000;
    /// Gas cost of roflmarket.ProviderUpdateOffers call.
    const GAS_COST_CALL_PROVIDER_UPDATE_OFFERS_BASE: u64 = 100_000;
    /// Gas cost of each added offer in roflmarket.ProviderUpdateOffers call.
    const GAS_COST_CALL_PROVIDER_UPDATE_OFFERS_ADD: u64 = 10_000;
    /// Gas cost of each removed offer in roflmarket.ProviderUpdateOffers call.
    const GAS_COST_CALL_PROVIDER_UPDATE_OFFERS_RM: u64 = 1_000;
    /// Gas cost of roflmarket.ProviderRemove call.
    const GAS_COST_CALL_PROVIDER_REMOVE: u64 = 100_000;
    /// Gas cost of roflmarket.InstanceCreate call.
    const GAS_COST_CALL_INSTANCE_CREATE: u64 = 100_000;
    /// Gas cost of roflmarket.InstanceAccept call.
    const GAS_COST_CALL_INSTANCE_ACCEPT_BASE: u64 = 10_000;
    /// Gas cost of each accepted instance in roflmarket.InstanceAccept call.
    const GAS_COST_CALL_INSTANCE_ACCEPT_INSTANCE: u64 = 10_000;
    /// Gas cost of roflmarket.InstanceTopUp call.
    const GAS_COST_CALL_INSTANCE_TOPUP: u64 = 10_000;
    /// Gas cost of roflmarket.InstanceUpdate call.
    const GAS_COST_CALL_INSTANCE_UPDATE_BASE: u64 = 10_000;
    /// Gas cost of each instance update in roflmarket.InstanceUpdate call.
    const GAS_COST_CALL_INSTANCE_UPDATE_INST: u64 = 10_000;
    /// Gas cost of roflmarket.InstanceCancel call.
    const GAS_COST_CALL_INSTANCE_CANCEL: u64 = 10_000;
    /// Gas cost of roflmarket.InstanceRemove call.
    const GAS_COST_CALL_INSTANCE_REMOVE: u64 = 10_000;
    /// Gas cost of roflmarket.InstanceExecuteCmds call.
    const GAS_COST_CALL_INSTANCE_EXECUTE_CMDS_BASE: u64 = 10_000;
    /// Gas cost of each command in roflmarket.InstanceExecuteCmds call.
    const GAS_COST_CALL_INSTANCE_EXECUTE_CMDS_CMD: u64 = 10_000;
    /// Gas cost of roflmarket.InstanceClaimPayment call.
    const GAS_COST_CALL_INSTANCE_CLAIM_PAYMENT_BASE: u64 = 10_000;
    /// Gas cost of each instance in roflmarket.InstanceClaimPayment call.
    const GAS_COST_CALL_INSTANCE_CLAIM_PAYMENT_INST: u64 = 10_000;

    /// Maximum time for a provider to accept an instance. If not accepted within this window, the
    /// instance may be cancelled and will be refunded.
    const MAX_INSTANCE_ACCEPT_TIME_SECONDS: u64 = 300;
    /// Maximum number of offers a provider can have.
    const MAX_PROVIDER_OFFERS: u64 = 64;
    /// Maximum number of queued instance commands.
    const MAX_QUEUED_INSTANCE_COMMANDS: u64 = 8;
    /// Maximum size of an instance command.
    const MAX_INSTANCE_COMMAND_SIZE: usize = 16 * 1024;

    /// Maximum number of metadata key-value pairs.
    const MAX_METADATA_PAIRS: usize = 64;
    /// Maximum metadata key size.
    const MAX_METADATA_KEY_SIZE: usize = 1024;
    /// Maximum metadata value size.
    const MAX_METADATA_VALUE_SIZE: usize = 16 * 1024;

    /// Amount of stake required for maintaining a provider.
    ///
    /// The stake is held in escrow and is returned to the provider when the entry is removed.
    const STAKE_PROVIDER_CREATE: token::BaseUnits =
        token::BaseUnits::new(0, token::Denomination::NATIVE);
}
