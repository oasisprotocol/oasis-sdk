use ethabi::{ParamType, Token};
use evm::{
    executor::stack::{PrecompileFailure, PrecompileHandle, PrecompileOutput},
    ExitError, ExitSucceed,
};

use crate::backend::EVMBackendExt;
use oasis_runtime_sdk::{
    module::CallResult, modules::core::Error, subcall, types::transaction::CallerAddress,
};

use super::{record_linear_cost, PrecompileResult};

/// A subcall validator which prevents any subcalls from re-entering the EVM module.
struct ForbidReentrancy;

impl subcall::Validator for ForbidReentrancy {
    fn validate(&self, info: &subcall::SubcallInfo) -> Result<(), Error> {
        if info.method.starts_with("evm.") {
            return Err(Error::Forbidden);
        }
        Ok(())
    }
}

const SUBCALL_BASE_COST: u64 = 10;
const SUBCALL_WORD_COST: u64 = 1;

pub(super) fn call_subcall<B: EVMBackendExt>(
    handle: &mut impl PrecompileHandle,
    backend: &B,
) -> PrecompileResult {
    record_linear_cost(
        handle,
        handle.input().len() as u64,
        SUBCALL_BASE_COST,
        SUBCALL_WORD_COST,
    )?;

    // Ensure that the precompile is called using a regular call (and not a delegatecall) so the
    // caller is actually the address of the calling contract.
    if handle.context().address != handle.code_address() {
        return Err(PrecompileFailure::Error {
            exit_status: ExitError::Other("invalid call".into()),
        });
    }

    let mut call_args = ethabi::decode(
        &[
            ParamType::Bytes, // method
            ParamType::Bytes, // body (CBOR)
        ],
        handle.input(),
    )
    .map_err(|e| PrecompileFailure::Error {
        exit_status: ExitError::Other(e.to_string().into()),
    })?;

    // Parse raw arguments.
    let body = call_args.pop().unwrap().into_bytes().unwrap();
    let method = call_args.pop().unwrap().into_bytes().unwrap();

    // Parse body as CBOR.
    let body = cbor::from_slice(&body).map_err(|_| PrecompileFailure::Error {
        exit_status: ExitError::Other("body is malformed".into()),
    })?;

    // Parse method.
    let method = String::from_utf8(method).map_err(|_| PrecompileFailure::Error {
        exit_status: ExitError::Other("method is malformed".into()),
    })?;

    // Cap maximum amount of gas that can be used.
    let max_gas = handle.remaining_gas();

    let result = backend
        .subcall(
            subcall::SubcallInfo {
                caller: CallerAddress::EthAddress(handle.context().caller.into()),
                method,
                body,
                max_depth: 8,
                max_gas,
            },
            ForbidReentrancy,
        )
        .map_err(|_| PrecompileFailure::Error {
            exit_status: ExitError::Other("subcall failed".into()),
        })?;

    // Charge gas (this shouldn't fail given that we set the limit appropriately).
    handle.record_cost(result.gas_used)?;

    match result.call_result {
        CallResult::Ok(value) => Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            output: ethabi::encode(&[
                Token::Uint(0.into()),             // status_code
                Token::Bytes(cbor::to_vec(value)), // response
            ]),
        }),
        CallResult::Failed { code, module, .. } => Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            output: ethabi::encode(&[
                Token::Uint(code.into()),    // status_code
                Token::Bytes(module.into()), // response
            ]),
        }),
        CallResult::Aborted(_) => {
            // TODO: Should propagate abort.
            Err(PrecompileFailure::Error {
                exit_status: ExitError::Other("subcall failed".into()),
            })
        }
    }
}

#[cfg(test)]
mod test {
    use base64::prelude::*;
    use ethabi::{ParamType, Token};

    use oasis_runtime_sdk::{
        module::{self, Module as _},
        modules::accounts,
        testing::{
            keys,
            mock::{CallOptions, Mock},
        },
        types::{
            address::Address,
            token::{self, BaseUnits, Denomination},
            transaction::Fee,
        },
    };

    use crate::{
        self as evm,
        mock::{decode_reverted, EvmSigner},
        precompile::testing::{init_and_deploy_contract, TestConfig, TestRuntime},
        Config as _,
    };

    /// Test contract code.
    static TEST_CONTRACT_CODE_HEX: &str =
        include_str!("../../../../../tests/e2e/evm/contracts/subcall/evm_subcall.hex");
    /// Test contract ABI.
    static TEST_CONTRACT_ABI_JSON: &str =
        include_str!("../../../../../tests/e2e/evm/contracts/subcall/evm_subcall.abi");

    #[test]
    fn test_subcall_dispatch() {
        let mut mock = Mock::default();
        let ctx = mock.create_ctx_for_runtime::<TestRuntime>(false);
        let mut signer = EvmSigner::new(0, keys::dave::sigspec());

        // Create contract.
        let contract_address = init_and_deploy_contract(&ctx, &mut signer, TEST_CONTRACT_CODE_HEX);

        // Call into the test contract.
        let dispatch_result = signer.call_evm(
            &ctx,
            contract_address.into(),
            "test",
            &[
                ParamType::Bytes, // method
                ParamType::Bytes, // body
            ],
            &[
                Token::Bytes("accounts.Transfer".into()),
                Token::Bytes(cbor::to_vec(accounts::types::Transfer {
                    to: keys::alice::address(),
                    amount: BaseUnits::new(1_000, Denomination::NATIVE),
                })),
            ],
        );
        assert!(
            !dispatch_result.result.is_success(),
            "call should fail due to insufficient balance"
        );

        // Transfer some tokens to the contract.
        let dispatch_result = signer.call(
            &ctx,
            "accounts.Transfer",
            accounts::types::Transfer {
                to: TestConfig::map_address(contract_address.into()),
                amount: BaseUnits::new(2_000, Denomination::NATIVE),
            },
        );
        assert!(
            dispatch_result.result.is_success(),
            "transfer should succeed"
        );

        // Call into test contract again.
        let dispatch_result = signer.call_evm_opts(
            &ctx,
            contract_address.into(),
            "test",
            &[
                ParamType::Bytes, // method
                ParamType::Bytes, // body
            ],
            &[
                Token::Bytes("accounts.Transfer".into()),
                Token::Bytes(cbor::to_vec(accounts::types::Transfer {
                    to: keys::alice::address(),
                    amount: BaseUnits::new(1_000, Denomination::NATIVE),
                })),
            ],
            CallOptions {
                fee: Fee {
                    amount: BaseUnits::new(100, Denomination::NATIVE),
                    gas: 1_000_000,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        assert!(dispatch_result.result.is_success(), "call should succeed");

        // Make sure two events were emitted and are properly formatted.
        let tags = &dispatch_result.tags;
        assert_eq!(tags.len(), 2, "two events should have been emitted");
        assert_eq!(tags[0].key, b"accounts\x00\x00\x00\x01"); // accounts.Transfer (code = 1) event
        assert_eq!(tags[1].key, b"core\x00\x00\x00\x01"); // core.GasUsed (code = 1) event

        #[derive(Debug, Default, cbor::Decode)]
        struct TransferEvent {
            from: Address,
            to: Address,
            amount: token::BaseUnits,
        }

        let events: Vec<TransferEvent> = cbor::from_slice(&tags[0].value).unwrap();
        assert_eq!(events.len(), 2); // One event for subcall, other event for fee payment.
        let event = &events[0];
        assert_eq!(event.from, Address::from_eth(contract_address.as_ref()));
        assert_eq!(event.to, keys::alice::address());
        assert_eq!(event.amount, BaseUnits::new(1_000, Denomination::NATIVE));
        let event = &events[1];
        assert_eq!(event.from, keys::dave::address());
        assert_eq!(event.amount, BaseUnits::new(100, Denomination::NATIVE));

        // Make sure only one gas used event was emitted (e.g. subcall should not emit its own gas
        // used events).
        #[derive(Debug, Default, cbor::Decode)]
        struct GasUsedEvent {
            amount: u64,
        }

        let events: Vec<GasUsedEvent> = cbor::from_slice(&tags[1].value).unwrap();
        assert_eq!(events.len(), 1); // Just one gas used event.
        assert_eq!(events[0].amount, 25742);
    }

    #[test]
    fn test_require_regular_call() {
        let mut mock = Mock::default();
        let ctx = mock.create_ctx_for_runtime::<TestRuntime>(false);
        let mut signer = EvmSigner::new(0, keys::dave::sigspec());

        // Create contract.
        let contract_address = init_and_deploy_contract(&ctx, &mut signer, TEST_CONTRACT_CODE_HEX);

        // Call into the test contract.
        let dispatch_result = signer.call_evm(
            &ctx,
            contract_address.into(),
            "test_delegatecall",
            &[
                ParamType::Bytes, // method
                ParamType::Bytes, // body
            ],
            &[
                Token::Bytes("accounts.Transfer".into()),
                Token::Bytes(cbor::to_vec(accounts::types::Transfer {
                    to: keys::alice::address(),
                    amount: BaseUnits::new(0, Denomination::NATIVE),
                })),
            ],
        );
        if let module::CallResult::Failed {
            module,
            code,
            message,
        } = dispatch_result.result
        {
            assert_eq!(module, "evm");
            assert_eq!(code, 8);
            assert_eq!(decode_reverted(&message).unwrap(), "subcall failed");
        } else {
            panic!("call should fail due to delegatecall");
        }
    }

    #[test]
    fn test_no_reentrance() {
        let mut mock = Mock::default();
        let ctx = mock.create_ctx_for_runtime::<TestRuntime>(false);
        let mut signer = EvmSigner::new(0, keys::dave::sigspec());

        // Create contract.
        let contract_address = init_and_deploy_contract(&ctx, &mut signer, TEST_CONTRACT_CODE_HEX);

        // Call into the test contract.
        let dispatch_result = signer.call_evm(
            &ctx,
            contract_address.into(),
            "test",
            &[
                ParamType::Bytes, // method
                ParamType::Bytes, // body
            ],
            &[
                Token::Bytes("evm.Call".into()),
                Token::Bytes(cbor::to_vec(evm::types::Call {
                    address: contract_address.into(),
                    value: 0.into(),
                    data: [
                        ethabi::short_signature("test", &[ParamType::Bytes, ParamType::Bytes])
                            .to_vec(),
                        ethabi::encode(&[
                            Token::Bytes("accounts.Transfer".into()),
                            Token::Bytes(cbor::to_vec(accounts::types::Transfer {
                                to: keys::alice::address(),
                                amount: BaseUnits::new(0, Denomination::NATIVE),
                            })),
                        ]),
                    ]
                    .concat(),
                })),
            ],
        );
        if let module::CallResult::Failed {
            module,
            code,
            message,
        } = dispatch_result.result
        {
            assert_eq!(module, "evm");
            assert_eq!(code, 8);
            assert_eq!(decode_reverted(&message).unwrap(), "subcall failed");
        } else {
            panic!("call should fail due to re-entrancy not being allowed");
        }
    }

    #[test]
    fn test_gas_accounting() {
        let mut mock = Mock::default();
        let ctx = mock.create_ctx_for_runtime::<TestRuntime>(false);
        let mut signer = EvmSigner::new(0, keys::dave::sigspec());

        // Create contract.
        let contract_address = init_and_deploy_contract(&ctx, &mut signer, TEST_CONTRACT_CODE_HEX);

        // Make transfers more expensive so we can test an out-of-gas condition.
        accounts::Module::set_params(accounts::Parameters {
            gas_costs: accounts::GasCosts {
                tx_transfer: 100_000,
            },
            ..Default::default()
        });

        // First try a call with enough gas.
        let dispatch_result = signer.call_evm_opts(
            &ctx,
            contract_address.into(),
            "test",
            &[
                ParamType::Bytes, // method
                ParamType::Bytes, // body
            ],
            &[
                Token::Bytes("accounts.Transfer".into()),
                Token::Bytes(cbor::to_vec(accounts::types::Transfer {
                    to: keys::alice::address(),
                    amount: BaseUnits::new(0, Denomination::NATIVE),
                })),
            ],
            CallOptions {
                fee: Fee {
                    gas: 130_000,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        assert!(
            dispatch_result.result.is_success(),
            "call with enough gas should succeed"
        );

        // Then lower the amount such that the inner call would fail, but the rest of execution
        // can still continue (e.g. to trigger the revert).
        let dispatch_result = signer.call_evm_opts(
            &ctx,
            contract_address.into(),
            "test",
            &[
                ParamType::Bytes, // method
                ParamType::Bytes, // body
            ],
            &[
                Token::Bytes("accounts.Transfer".into()),
                Token::Bytes(cbor::to_vec(accounts::types::Transfer {
                    to: keys::alice::address(),
                    amount: BaseUnits::new(0, Denomination::NATIVE),
                })),
            ],
            CallOptions {
                fee: Fee {
                    gas: 120_000,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        if let module::CallResult::Failed {
            module,
            code,
            message,
        } = dispatch_result.result
        {
            assert_eq!(module, "evm");
            assert_eq!(code, 8);

            let message = message.strip_prefix("reverted: ").unwrap();
            let data = BASE64_STANDARD.decode(message).unwrap();
            let abi = ethabi::Contract::load(TEST_CONTRACT_ABI_JSON.as_bytes()).unwrap();
            let mut err = abi
                .error("SubcallFailed")
                .unwrap()
                .decode(&data[4..])
                .unwrap();

            let subcall_module = err.pop().unwrap().into_bytes().unwrap();
            let subcall_code: u64 = err.pop().unwrap().into_uint().unwrap().try_into().unwrap();

            assert_eq!(subcall_module, "core".as_bytes());
            assert_eq!(subcall_code, 12); // Error code 12 for module core is "out of gas".
        } else {
            panic!("call should fail due to subcall running out of gas");
        }

        // Then raise the amount such that the inner call would succeed but the rest of the
        // execution would fail.
        let dispatch_result = signer.call_evm_opts(
            &ctx,
            contract_address.into(),
            "test_spin", // Version that spins, wasting gas, after the subcall.
            &[
                ParamType::Bytes, // method
                ParamType::Bytes, // body
            ],
            &[
                Token::Bytes("accounts.Transfer".into()),
                Token::Bytes(cbor::to_vec(accounts::types::Transfer {
                    to: keys::alice::address(),
                    amount: BaseUnits::new(0, Denomination::NATIVE),
                })),
            ],
            CallOptions {
                fee: Fee {
                    gas: 127_710,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        if let module::CallResult::Failed {
            module,
            code,
            message,
        } = dispatch_result.result
        {
            assert_eq!(module, "evm");
            assert_eq!(code, 2);
            assert_eq!(message, "execution failed: out of gas");
        } else {
            panic!("call should fail due to running out of gas");
        }
    }
}
