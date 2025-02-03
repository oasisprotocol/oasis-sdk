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

/// Helper trait for emitting EVM events.
///
/// Contracts that need to emit events should do so by encapsulating them in
/// structs which derive from it. The derive macro will generate code to
/// automatically encode event parameters into the format required by the EVM.
pub trait EvmEvent {
    /// Encode the event's data into the required EVM log format end emit it.
    fn emit<C: StaticContract>(&self, handle: &mut impl PrecompileHandle) -> Result<(), ExitError>;
}

/// Helper trait for raising errors.
///
/// Contracts that have custom errors as part of their API should implement
/// them as an enum which derives from this trait. The derive macro will
/// provide automatic encoding based on error signatures.
pub trait EvmError {
    /// Encode the error's type and parameters into the format specified by the
    /// EVM ABI and wrap it into a failure sruct used by the executor.
    fn encode(&self) -> PrecompileFailure;
}
