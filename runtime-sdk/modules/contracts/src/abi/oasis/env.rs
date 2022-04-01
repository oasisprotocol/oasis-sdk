//! Environment query imports.
use oasis_contract_sdk_types::{
    env::{AccountsQuery, AccountsResponse, QueryRequest, QueryResponse},
    InstanceId,
};
use oasis_runtime_sdk::{context::Context, modules::accounts::API as _};

use super::{memory::Region, OasisV1};
use crate::{
    abi::{gas, ExecutionContext},
    types::Instance,
    Config, Error,
};

impl<Cfg: Config> OasisV1<Cfg> {
    /// Link environment query functions.
    pub fn link_env<C: Context>(
        instance: &mut wasm3::Instance<'_, '_, ExecutionContext<'_, C>>,
    ) -> Result<(), Error> {
        // env.query(request) -> response
        let _ = instance.link_function(
            "env",
            "query",
            |ctx, query: (u32, u32)| -> Result<u32, wasm3::Trap> {
                // Make sure function was called in valid context.
                let ec = ctx.context.ok_or(wasm3::Trap::Abort)?;

                // Charge base gas amount.
                gas::use_gas(ctx.instance, ec.params.gas_costs.wasm_env_query_base)?;

                // Decode query argument.
                let request: QueryRequest = ctx.instance.runtime().try_with_memory(
                    |memory| -> Result<_, wasm3::Trap> {
                        let query = Region::from_arg(query).as_slice(&memory)?;
                        if query.len() > ec.params.max_query_size_bytes as usize {
                            // TODO: Consider returning a nicer error message.
                            return Err(wasm3::Trap::Abort);
                        }

                        cbor::from_slice(query).map_err(|_| wasm3::Trap::Abort)
                    },
                )??;

                // Dispatch query.
                let result = dispatch_query::<Cfg, C>(ec.tx_context, request);

                // Create new region by calling `allocate`.
                //
                // This makes sure that the call context is unset to avoid any potential issues
                // with reentrancy as attempting to re-enter one of the linked function will fail.
                Self::serialize_and_allocate_as_ptr(ctx.instance, result).map_err(|err| err.into())
            },
        );

        // env.address_for_instance(instance_id, dst_region)
        let _ = instance.link_function(
            "env",
            "address_for_instance",
            |ctx, request: (u64, (u32, u32))| -> Result<(), wasm3::Trap> {
                // Make sure function was called in valid context.
                let ec = ctx.context.ok_or(wasm3::Trap::Abort)?;

                // Charge base gas amount.
                // TODO: probably separate gas cost.
                gas::use_gas(ctx.instance, ec.params.gas_costs.wasm_env_query_base)?;

                ctx.instance
                    .runtime()
                    .try_with_memory(|mut memory| -> Result<_, wasm3::Trap> {
                        let instance_id: InstanceId = request.0.into();
                        let dst = Region::from_arg(request.1).as_slice_mut(&mut memory)?;

                        let address = Instance::address_for(instance_id);

                        dst.copy_from_slice(address.as_ref());

                        Ok(())
                    })?
            },
        );

        // env.debug_print(messsage, len)
        #[cfg(feature = "debug-utils")]
        let _ = instance.link_function(
            "env",
            "debug_print",
            |ctx, request: (u32, u32)| -> Result<(), wasm3::Trap> {
                ctx.instance
                    .runtime()
                    .try_with_memory(|memory| -> Result<_, wasm3::Trap> {
                        let msg_bytes = Region::from_arg(request).as_slice(&memory)?;
                        if let Ok(msg) = std::str::from_utf8(msg_bytes) {
                            eprintln!("{}", msg);
                        }
                        Ok(())
                    })?
            },
        );

        Ok(())
    }
}

/// Perform environment query dispatch.
fn dispatch_query<Cfg: Config, C: Context>(ctx: &mut C, query: QueryRequest) -> QueryResponse {
    match query {
        // Information about the current runtime block.
        QueryRequest::BlockInfo => QueryResponse::BlockInfo {
            round: ctx.runtime_header().round,
            epoch: ctx.epoch(),
            timestamp: ctx.runtime_header().timestamp,
        },

        // Accounts API queries.
        QueryRequest::Accounts(query) => dispatch_accounts_query::<Cfg, C>(ctx, query),

        _ => QueryResponse::Error {
            module: "".to_string(),
            code: 1,
            message: "query not supported".to_string(),
        },
    }
}

/// Perform accounts API query dispatch.
fn dispatch_accounts_query<Cfg: Config, C: Context>(
    ctx: &mut C,
    query: AccountsQuery,
) -> QueryResponse {
    match query {
        AccountsQuery::Balance {
            address,
            denomination,
        } => {
            let balance = Cfg::Accounts::get_balance(
                ctx.runtime_state(),
                address.into(),
                denomination.into(),
            )
            .unwrap_or_default();

            AccountsResponse::Balance { balance }.into()
        }

        _ => QueryResponse::Error {
            module: "".to_string(),
            code: 1,
            message: "query not supported".to_string(),
        },
    }
}
