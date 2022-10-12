//! The Oasis ABIs.
use oasis_contract_sdk_types as contract_sdk;
use oasis_runtime_sdk::{context::Context, modules::core, runtime::Runtime, types::token};

use super::{gas, Abi, ExecutionContext, ExecutionResult, Info};
use crate::{wasm::ContractError, Config, Error, Parameters};

mod crypto;
mod env;
mod memory;
mod storage;
#[cfg(test)]
mod test;
mod validation;

const EXPORT_INSTANTIATE: &str = "instantiate";
const EXPORT_CALL: &str = "call";
const EXPORT_HANDLE_REPLY: &str = "handle_reply";
const EXPORT_PRE_UPGRADE: &str = "pre_upgrade";
const EXPORT_POST_UPGRADE: &str = "post_upgrade";
const EXPORT_QUERY: &str = "query";

const GAS_SCALING_FACTOR: u64 = 1;

/// The Oasis V1 ABI.
pub struct OasisV1<Cfg: Config> {
    _cfg: std::marker::PhantomData<Cfg>,
}

impl<Cfg: Config> OasisV1<Cfg> {
    /// The set of required exports.
    const REQUIRED_EXPORTS: &'static [&'static str] = &[
        memory::EXPORT_ALLOCATE,
        memory::EXPORT_DEALLOCATE,
        EXPORT_INSTANTIATE,
        EXPORT_CALL,
    ];

    /// The set of reserved exports.
    const RESERVED_EXPORTS: &'static [&'static str] =
        &[gas::EXPORT_GAS_LIMIT, gas::EXPORT_GAS_LIMIT_EXHAUSTED];

    /// Create a new instance of the ABI.
    pub fn new() -> Self {
        Self {
            _cfg: std::marker::PhantomData,
        }
    }

    fn raw_call_with_request_context<'ctx, C: Context>(
        ctx: &mut ExecutionContext<'ctx, C>,
        instance: &wasm3::Instance<'_, '_, ExecutionContext<'ctx, C>>,
        request: &[u8],
        deposited_tokens: &[token::BaseUnits],
        function_name: &str,
    ) -> Result<contract_sdk::ExecutionOk, Error> {
        // Allocate memory for context and request, copy serialized data into the region.
        let mut ec = contract_sdk::ExecutionContext {
            instance_id: ctx.instance_info.id,
            instance_address: ctx.instance_info.address().into(),
            caller_address: ctx.caller_address.into(),
            deposited_tokens: deposited_tokens.iter().map(|b| b.into()).collect(),
            ..Default::default()
        };
        if ctx.code_info.abi_sv >= 1 {
            // Supports read only and call format flags.
            ec.read_only = ctx.read_only;
            ec.call_format = ctx.call_format.into();
        }
        let context_dst = Self::serialize_and_allocate(instance, ec)
            .map_err(|err| Error::ExecutionFailed(err.into()))?;
        let request_dst = Self::allocate_and_copy(instance, request)
            .map_err(|err| Error::ExecutionFailed(err.into()))?;

        // Call the corresponding function in the smart contract.
        let result = {
            // The high-level function signature of the WASM export is as follows:
            //
            //   fn(ctx: &contract_sdk::ExecutionContext, request: &[u8]) -> contract_sdk::ExecutionResult
            //
            let func = instance
                .find_function::<((u32, u32), (u32, u32)), u32>(function_name)
                .map_err(|err| Error::ExecutionFailed(err.into()))?;
            let result = func
                .call_with_context(ctx, (context_dst.to_arg(), request_dst.to_arg()))
                .map_err(|err| Error::ExecutionFailed(err.into()))?;
            instance
                .runtime()
                .try_with_memory(|memory| -> Result<_, Error> {
                    memory::Region::deref(&memory, result)
                        .map_err(|err| Error::ExecutionFailed(err.into()))
                })
                .unwrap()?
        };

        // Enforce maximum result size limit before attempting to deserialize it.
        if result.length as u32 > ctx.params.max_result_size_bytes {
            return Err(Error::ResultTooLarge(
                result.length as u32,
                ctx.params.max_result_size_bytes,
            ));
        }

        // Deserialize region into result structure.
        let result: contract_sdk::ExecutionResult = instance
            .runtime()
            .try_with_memory(|memory| -> Result<_, Error> {
                let data = result
                    .as_slice(&memory)
                    .map_err(|err| Error::ExecutionFailed(err.into()))?;

                cbor::from_slice(data).map_err(|err| Error::ExecutionFailed(err.into()))
            })
            .unwrap()?;

        match result {
            contract_sdk::ExecutionResult::Ok(ok) => Ok(ok),
            contract_sdk::ExecutionResult::Failed {
                module,
                code,
                message,
            } => Err(ContractError::new(ctx.instance_info.code_id, &module, code, &message).into()),
        }
    }

    fn call_with_request_context<'ctx, C: Context>(
        ctx: &mut ExecutionContext<'ctx, C>,
        instance: &wasm3::Instance<'_, '_, ExecutionContext<'ctx, C>>,
        request: &[u8],
        deposited_tokens: &[token::BaseUnits],
        function_name: &str,
    ) -> ExecutionResult {
        // Fetch initial gas counter value so we can determine how much gas was used.
        let initial_gas = gas::get_remaining_gas(instance);

        let inner = Self::raw_call_with_request_context(
            ctx,
            instance,
            request,
            deposited_tokens,
            function_name,
        )
        .map_err(|err| {
            // Check if an abort flag has been set and propagate the error.
            if let Some(aborted) = ctx.aborted.take() {
                return aborted;
            }

            // Check if call failed due to gas being exhausted and return a proper error.
            let exhausted_gas = gas::get_exhausted_amount(instance);
            if exhausted_gas != 0 {
                // Compute how much gas was wanted.
                let final_gas = gas::get_remaining_gas(instance);
                let wanted_gas = initial_gas + exhausted_gas.saturating_sub(final_gas);
                core::Error::out_of_gas::<<<C::Runtime as Runtime>::Core as core::API>::Config>(
                    initial_gas,
                    wanted_gas,
                )
                .into()
            } else {
                err
            }
        });

        // Compute how much gas (in SDK units) was actually used.
        let final_gas = gas::get_remaining_gas(instance);
        let gas_used = initial_gas.saturating_sub(final_gas) / GAS_SCALING_FACTOR;

        ExecutionResult { inner, gas_used }
    }
}

impl<Cfg: Config, C: Context> Abi<C> for OasisV1<Cfg> {
    fn validate(&self, module: &mut walrus::Module, params: &Parameters) -> Result<Info, Error> {
        let info = self.validate_module(module, params)?;

        // Add gas metering instrumentation.
        gas::transform(module);

        Ok(info)
    }

    fn link(
        &self,
        instance: &mut wasm3::Instance<'_, '_, ExecutionContext<'_, C>>,
    ) -> Result<(), Error> {
        // Storage imports.
        Self::link_storage(instance)?;
        // Environment imports.
        Self::link_env(instance)?;
        // Crypto imports.
        Self::link_crypto(instance)?;

        Ok(())
    }

    fn set_gas_limit(
        &self,
        instance: &mut wasm3::Instance<'_, '_, ExecutionContext<'_, C>>,
        gas_limit: u64,
    ) -> Result<(), Error> {
        // Derive gas limit from remaining transaction gas based on a scaling factor.
        let gas_limit = gas_limit.saturating_mul(GAS_SCALING_FACTOR);
        gas::set_gas_limit(instance, gas_limit)?;

        Ok(())
    }

    fn instantiate<'ctx>(
        &self,
        ctx: &mut ExecutionContext<'ctx, C>,
        instance: &wasm3::Instance<'_, '_, ExecutionContext<'ctx, C>>,
        request: &[u8],
        deposited_tokens: &[token::BaseUnits],
    ) -> ExecutionResult {
        Self::call_with_request_context(
            ctx,
            instance,
            request,
            deposited_tokens,
            EXPORT_INSTANTIATE,
        )
    }

    fn call<'ctx>(
        &self,
        ctx: &mut ExecutionContext<'ctx, C>,
        instance: &wasm3::Instance<'_, '_, ExecutionContext<'ctx, C>>,
        request: &[u8],
        deposited_tokens: &[token::BaseUnits],
    ) -> ExecutionResult {
        Self::call_with_request_context(ctx, instance, request, deposited_tokens, EXPORT_CALL)
    }

    fn handle_reply<'ctx>(
        &self,
        ctx: &mut ExecutionContext<'ctx, C>,
        instance: &wasm3::Instance<'_, '_, ExecutionContext<'ctx, C>>,
        reply: contract_sdk::message::Reply,
    ) -> ExecutionResult {
        Self::call_with_request_context(
            ctx,
            instance,
            &cbor::to_vec(reply),
            &[],
            EXPORT_HANDLE_REPLY,
        )
    }

    fn pre_upgrade<'ctx>(
        &self,
        ctx: &mut ExecutionContext<'ctx, C>,
        instance: &wasm3::Instance<'_, '_, ExecutionContext<'ctx, C>>,
        request: &[u8],
        deposited_tokens: &[token::BaseUnits],
    ) -> ExecutionResult {
        Self::call_with_request_context(
            ctx,
            instance,
            request,
            deposited_tokens,
            EXPORT_PRE_UPGRADE,
        )
    }

    fn post_upgrade<'ctx>(
        &self,
        ctx: &mut ExecutionContext<'ctx, C>,
        instance: &wasm3::Instance<'_, '_, ExecutionContext<'ctx, C>>,
        request: &[u8],
        deposited_tokens: &[token::BaseUnits],
    ) -> ExecutionResult {
        Self::call_with_request_context(
            ctx,
            instance,
            request,
            deposited_tokens,
            EXPORT_POST_UPGRADE,
        )
    }

    fn query<'ctx>(
        &self,
        ctx: &mut ExecutionContext<'ctx, C>,
        instance: &wasm3::Instance<'_, '_, ExecutionContext<'ctx, C>>,
        request: &[u8],
    ) -> ExecutionResult {
        Self::call_with_request_context(ctx, instance, request, &[], EXPORT_QUERY)
    }
}
