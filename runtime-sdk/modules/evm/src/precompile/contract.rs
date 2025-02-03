use evm::{
    executor::stack::{PrecompileFailure, PrecompileFn, PrecompileOutput},
    Context,
};

/// A static contract is a contract whose implementation is provided by the
/// runtime, much like the runtime provides function implementations at
/// well-known addresses as precompiles.
pub trait StaticContract {
    /// Dispatch a contract method call to a particular method of the
    /// implementing struct.
    fn dispatch_call(
        input: &[u8],
        gas_limit: Option<u64>,
        ctx: &Context,
        is_static: bool,
    ) -> Result<(PrecompileOutput, u64), PrecompileFailure>;

    fn as_precompile() -> PrecompileFn {
        Self::dispatch_call
    }
}
