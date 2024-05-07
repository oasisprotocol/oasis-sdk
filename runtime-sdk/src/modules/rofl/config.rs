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

    /// Amount of stake required for maintaining an application.
    ///
    /// The stake is held in escrow and is returned to the administrator when the application is
    /// removed.
    const STAKE_APP_CREATE: token::BaseUnits =
        token::BaseUnits::new(0, token::Denomination::NATIVE);
}
