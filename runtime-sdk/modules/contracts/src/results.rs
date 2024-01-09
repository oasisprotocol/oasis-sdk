//! Processing of execution results.
use std::convert::TryInto;

use oasis_contract_sdk_types::{
    event::Event,
    message::{Message, NotifyReply, Reply},
    ExecutionOk,
};
use oasis_runtime_sdk::{
    context::Context,
    event::etag_for_event,
    modules::core::API as _,
    runtime::Runtime,
    state::CurrentState,
    subcall::{self, SubcallInfo},
    types::transaction::CallerAddress,
};

use crate::{
    abi::{ExecutionContext, ExecutionResult},
    types::ContractEvent,
    wasm, Config, Error, Parameters, MODULE_NAME,
};

/// Process an execution result by performing gas accounting and returning the inner result.
pub(crate) fn process_execution_result<C: Context>(
    _ctx: &C,
    result: ExecutionResult,
) -> Result<ExecutionOk, Error> {
    // The following call should never fail as we accounted for all the gas in advance.
    <C::Runtime as Runtime>::Core::use_tx_gas(result.gas_used)?;

    result.inner
}

/// Process a successful execution result.
pub(crate) fn process_execution_success<Cfg: Config, C: Context>(
    ctx: &C,
    params: &Parameters,
    contract: &wasm::Contract<'_>,
    result: ExecutionOk,
) -> Result<Vec<u8>, Error> {
    // Process events.
    process_events(contract, result.events)?;
    // Process subcalls.
    let result = process_subcalls::<Cfg, C>(ctx, params, contract, result.messages, result.data)?;

    Ok(result)
}

fn process_events(contract: &wasm::Contract<'_>, events: Vec<Event>) -> Result<(), Error> {
    // Transform contract events into tags using the SDK scheme.
    CurrentState::with(|state| {
        for event in events {
            state.emit_event_raw(etag_for_event(
                &if event.module.is_empty() {
                    format!("{}.{}", MODULE_NAME, contract.code_info.id.as_u64())
                } else {
                    format!(
                        "{}.{}.{}",
                        MODULE_NAME,
                        contract.code_info.id.as_u64(),
                        event.module,
                    )
                },
                event.code,
                cbor::to_value(ContractEvent {
                    id: contract.instance_info.id,
                    data: event.data,
                }),
            ));
        }
    });

    Ok(())
}

fn process_subcalls<Cfg: Config, C: Context>(
    ctx: &C,
    params: &Parameters,
    contract: &wasm::Contract<'_>,
    messages: Vec<Message>,
    data: Vec<u8>,
) -> Result<Vec<u8>, Error> {
    // By default the resulting data is what the call returned. Message reply processing may
    // overwrite this data when it is non-empty.
    let mut result_data = data;

    // Charge gas for each emitted message.
    <C::Runtime as Runtime>::Core::use_tx_gas(
        params
            .gas_costs
            .subcall_dispatch
            .saturating_mul(messages.len() as u64),
    )?;

    // Make sure the number of subcalls is within limits.
    let message_count = messages
        .len()
        .try_into()
        .map_err(|_| Error::TooManySubcalls(u16::MAX, params.max_subcall_count))?;
    if message_count > params.max_subcall_count {
        return Err(Error::TooManySubcalls(
            message_count,
            params.max_subcall_count,
        ));
    }

    // Properly propagate original call format and read-only flag.
    let (orig_call_format, orig_read_only) =
        CurrentState::with_env(|env| (env.tx_call_format(), env.is_read_only()));

    // Process emitted messages recursively.
    for msg in messages {
        match msg {
            Message::Call {
                id,
                data,
                reply,
                method,
                body,
                max_gas,
            } => {
                // Compute the amount of gas that can be used.
                let remaining_gas = <C::Runtime as Runtime>::Core::remaining_tx_gas();
                let max_gas = max_gas.unwrap_or(remaining_gas);
                let max_gas = if max_gas > remaining_gas {
                    remaining_gas
                } else {
                    max_gas
                };

                let result = subcall::call(
                    ctx,
                    SubcallInfo {
                        caller: CallerAddress::Address(contract.instance_info.address()),
                        method,
                        body,
                        max_depth: params.max_subcall_depth,
                        max_gas,
                    },
                    subcall::AllowAllValidator,
                )?;

                // Use any gas that was used inside the child context. This should never fail as we
                // preconfigured the amount of available gas.
                <C::Runtime as Runtime>::Core::use_tx_gas(result.gas_used)?;

                // Process replies based on filtering criteria.
                let result = result.call_result;
                match (reply, result.is_success()) {
                    (NotifyReply::OnError, false)
                    | (NotifyReply::OnSuccess, true)
                    | (NotifyReply::Always, _) => {
                        // Construct and process reply.
                        let reply = Reply::Call {
                            id,
                            result: result.into(),
                            data,
                        };
                        let mut exec_ctx = ExecutionContext::new(
                            params,
                            contract.code_info,
                            contract.instance_info,
                            <C::Runtime as Runtime>::Core::remaining_tx_gas(),
                            CurrentState::with_env(|env| env.tx_caller_address()),
                            orig_read_only,
                            orig_call_format,
                            ctx,
                        );
                        let reply_result =
                            wasm::handle_reply::<Cfg, C>(&mut exec_ctx, contract, reply);
                        let reply_result = process_execution_result(ctx, reply_result)?;
                        let reply_result = process_execution_success::<Cfg, C>(
                            ctx,
                            params,
                            contract,
                            reply_result,
                        )?;

                        // If there is a non-empty reply, it overwrites the returned data.
                        if !reply_result.is_empty() {
                            result_data = reply_result;
                        }
                    }
                    _ => {}
                }
            }

            // Message not supported.
            _ => return Err(Error::Unsupported),
        }
    }

    Ok(result_data)
}
