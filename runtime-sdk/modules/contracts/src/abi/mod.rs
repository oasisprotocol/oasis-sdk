//! WASM ABI supported by the contracts module.
use oasis_contract_sdk_types::{message::Reply, ExecutionOk};
use oasis_runtime_sdk::{
    context::Context,
    types::{address::Address, token, transaction::CallFormat},
};

use super::{types, Error, Parameters};

pub mod gas;
pub mod oasis;

/// Trait for any WASM ABI to implement.
pub trait Abi<C: Context> {
    /// Validate that the given WASM module conforms to the ABI.
    fn validate(&self, module: &mut walrus::Module, params: &Parameters) -> Result<Info, Error>;

    /// Link required functions into the WASM module instance.
    fn link(
        &self,
        instance: &mut wasm3::Instance<'_, '_, ExecutionContext<'_, C>>,
    ) -> Result<(), Error>;

    /// Set the gas limit for any following executions.
    ///
    /// The specified gas limit should be in regular SDK gas units, not in WASM gas units. The ABI
    /// should perform any necessary conversions if required.
    fn set_gas_limit(
        &self,
        instance: &mut wasm3::Instance<'_, '_, ExecutionContext<'_, C>>,
        gas_limit: u64,
    ) -> Result<(), Error>;

    /// Instantiate a contract.
    fn instantiate<'ctx>(
        &self,
        ctx: &mut ExecutionContext<'ctx, C>,
        instance: &wasm3::Instance<'_, '_, ExecutionContext<'ctx, C>>,
        request: &[u8],
        deposited_tokens: &[token::BaseUnits],
    ) -> ExecutionResult;

    /// Call a contract.
    fn call<'ctx>(
        &self,
        ctx: &mut ExecutionContext<'ctx, C>,
        instance: &wasm3::Instance<'_, '_, ExecutionContext<'ctx, C>>,
        request: &[u8],
        deposited_tokens: &[token::BaseUnits],
    ) -> ExecutionResult;

    /// Invoke the contract's reply handler.
    fn handle_reply<'ctx>(
        &self,
        ctx: &mut ExecutionContext<'ctx, C>,
        instance: &wasm3::Instance<'_, '_, ExecutionContext<'ctx, C>>,
        reply: Reply,
    ) -> ExecutionResult;

    /// Invoke the contract's pre-upgrade handler.
    fn pre_upgrade<'ctx>(
        &self,
        ctx: &mut ExecutionContext<'ctx, C>,
        instance: &wasm3::Instance<'_, '_, ExecutionContext<'ctx, C>>,
        request: &[u8],
        deposited_tokens: &[token::BaseUnits],
    ) -> ExecutionResult;

    /// Invoke the contract's post-upgrade handler.
    fn post_upgrade<'ctx>(
        &self,
        ctx: &mut ExecutionContext<'ctx, C>,
        instance: &wasm3::Instance<'_, '_, ExecutionContext<'ctx, C>>,
        request: &[u8],
        deposited_tokens: &[token::BaseUnits],
    ) -> ExecutionResult;

    /// Query a contract.
    fn query<'ctx>(
        &self,
        ctx: &mut ExecutionContext<'ctx, C>,
        instance: &wasm3::Instance<'_, '_, ExecutionContext<'ctx, C>>,
        request: &[u8],
    ) -> ExecutionResult;
}

/// Additional information related to the ABI instance.
pub struct Info {
    /// ABI sub-version number.
    pub abi_sv: u32,
}

/// Execution context.
pub struct ExecutionContext<'ctx, C: Context> {
    /// Transaction context.
    pub tx_context: &'ctx mut C,
    /// Contracts module parameters.
    pub params: &'ctx Parameters,

    /// Code information.
    pub code_info: &'ctx types::Code,
    /// Contract instance information.
    pub instance_info: &'ctx types::Instance,
    /// Gas limit for this contract execution.
    pub gas_limit: u64,

    /// Address of the caller.
    pub caller_address: Address,
    /// Whether the call is read-only and must not make any storage modifications.
    pub read_only: bool,
    /// Call format.
    pub call_format: CallFormat,

    /// Whether the execution has aborted with an error that should be propagated instead of just
    /// using the generic "execution failed" error.
    pub aborted: Option<Error>,
}

impl<'ctx, C: Context> ExecutionContext<'ctx, C> {
    /// Create a new execution context.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        params: &'ctx Parameters,
        code_info: &'ctx types::Code,
        instance_info: &'ctx types::Instance,
        gas_limit: u64,
        caller_address: Address,
        read_only: bool,
        call_format: CallFormat,
        tx_context: &'ctx mut C,
    ) -> Self {
        Self {
            tx_context,
            params,
            code_info,
            instance_info,
            gas_limit,
            caller_address,
            read_only,
            call_format,
            aborted: None,
        }
    }
}

/// Result of an execution that contains additional metadata like gas used.
#[must_use]
pub struct ExecutionResult {
    /// Actual execution result.
    pub inner: Result<ExecutionOk, Error>,
    /// Amount of gas used by the execution.
    pub gas_used: u64,
}
