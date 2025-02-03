use evm::{
    executor::stack::{PrecompileFailure, PrecompileHandle},
    ExitError,
};

use crate::precompile::PrecompileResult;

/// A static contract is a contract whose implementation is provided by the
/// runtime, much like the runtime provides function implementations at
/// well-known addresses as precompiles.
pub trait StaticContract {
    /// Return the address of this contract.
    fn address() -> ::primitive_types::H160;

    /// Dispatch a contract method call to a particular method of the
    /// implementing struct.
    fn dispatch_call(handle: &mut impl PrecompileHandle) -> Option<PrecompileResult>;
}

pub trait EvmEvent {
    fn emit<C: StaticContract>(&self, handle: &mut impl PrecompileHandle) -> Result<(), ExitError>;
}

pub trait EvmError {
    fn encode(&self) -> PrecompileFailure;
}
