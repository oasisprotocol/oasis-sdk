//! WASM ABI supported by the contracts module.
use super::{types, Error};

pub mod gas;
pub mod oasis;

/// Trait for any WASM ABI to implement.
pub trait ABI {
    /// Validate that the given WASM module conforms to the ABI.
    fn validate(&self, module: &mut walrus::Module) -> Result<(), Error>;

    /// Link required functions into the WASM module.
    fn link(&self, module: wasm3::Module<'_>) -> Result<(), Error>;

    /// Instantiate a contract.
    fn instantiate(
        &mut self,
        rt: &mut wasm3::Runtime,
        request: &[u8],
        instance_info: &types::Instance,
    ) -> Result<(), Error>;

    // Call a contract.
    fn call(
        &mut self,
        rt: &mut wasm3::Runtime,
        request: &[u8],
        instance_info: &types::Instance,
    ) -> Result<ExecutionOk, Error>;
}

/// Result of a successful contract execution.
#[derive(Clone, Debug)]
pub struct ExecutionOk {
    /// Raw data returned from the contract.
    pub data: Vec<u8>,
    // TODO: events, messages
}
