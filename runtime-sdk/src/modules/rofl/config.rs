use crate::types::token;

/// Module configuration.
pub trait Config: 'static {
    /// Gas cost of rofl.Create call.
    const GAS_COST_CALL_CREATE: u64 = 100_000;
    /// Gas cost of rofl.Update call.
    const GAS_COST_CALL_UPDATE: u64 = 100_000;
    /// Gas cost of rofl.Remove call.
    const GAS_COST_CALL_REMOVE: u64 = 10_000;
    /// Gas cost of rofl.Register call.
    const GAS_COST_CALL_REGISTER: u64 = 100_000;
    /// Gas cost of rofl.IsAuthorizedOrigin call.
    const GAS_COST_CALL_IS_AUTHORIZED_ORIGIN: u64 = 1000;
    /// Gas cost of rofl.AuthorizedOriginNode call.
    const GAS_COST_CALL_AUTHORIZED_ORIGIN_NODE: u64 = 2000;
    /// Gas cost of rofl.AuthorizedOriginEntity call.
    const GAS_COST_CALL_AUTHORIZED_ORIGIN_ENTITY: u64 = 2000;
    /// Gas cost of rofl.StakeThresholds call.
    const GAS_COST_CALL_STAKE_THRESHOLDS: u64 = 10;
    /// Gas cost of rofl.DeriveKey call.
    const GAS_COST_CALL_DERIVE_KEY: u64 = 10_000;

    /// Maximum number of metadata key-value pairs.
    const MAX_METADATA_PAIRS: usize = 64;
    /// Maximum metadata key size.
    const MAX_METADATA_KEY_SIZE: usize = 1024;
    /// Maximum metadata value size.
    const MAX_METADATA_VALUE_SIZE: usize = 16 * 1024;

    /// Amount of stake required for maintaining an application.
    ///
    /// The stake is held in escrow and is returned to the administrator when the application is
    /// removed.
    const STAKE_APP_CREATE: token::BaseUnits =
        token::BaseUnits::new(0, token::Denomination::NATIVE);

    /// Maximum key identifier length for rofl.DeriveKey call.
    const DERIVE_KEY_MAX_KEY_ID_LENGTH: usize = 128;
}
