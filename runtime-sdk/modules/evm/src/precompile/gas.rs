use ethabi::{ParamType, Token};
use evm::{
    executor::stack::{PrecompileFailure, PrecompileHandle, PrecompileOutput},
    ExitError, ExitSucceed,
};

use super::PrecompileResult;

const GAS_USED_COST: u64 = 10;
const PAD_GAS_COST: u64 = 10;

pub(super) fn call_gas_used(handle: &mut impl PrecompileHandle) -> PrecompileResult {
    handle.record_cost(GAS_USED_COST)?;

    let used_gas = handle.used_gas();

    Ok(PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        output: ethabi::encode(&[Token::Uint(used_gas.into())]),
    })
}

pub(super) fn call_pad_gas(handle: &mut impl PrecompileHandle) -> PrecompileResult {
    handle.record_cost(PAD_GAS_COST)?;

    // Decode args.
    let mut call_args = ethabi::decode(&[ParamType::Uint(128)], handle.input()).map_err(|e| {
        PrecompileFailure::Error {
            exit_status: ExitError::Other(e.to_string().into()),
        }
    })?;
    let gas_amount_big = call_args.pop().unwrap().into_uint().unwrap();
    let gas_amount = gas_amount_big.try_into().unwrap_or(u64::MAX);

    // Obtain total used gas so far.
    let used_gas = handle.used_gas();

    // Fail if more gas that the desired padding was already used.
    if gas_amount < used_gas {
        return Err(PrecompileFailure::Error {
            exit_status: ExitError::Other(
                "gas pad amount less than already used gas"
                    .to_string()
                    .into(),
            ),
        });
    }

    // Record the remainder so that the gas use is padded to the desired amount.
    handle.record_cost(gas_amount - used_gas)?;

    Ok(PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        output: Vec::new(),
    })
}

#[cfg(test)]
mod test {
    use super::super::testing::*;
    use crate::{
        mock::EvmSigner,
        precompile::testing::{init_and_deploy_contract, TestRuntime},
    };
    use ethabi::{ParamType, Token};
    use oasis_runtime_sdk::{
        modules::core::Event,
        testing::{keys, mock::Mock},
    };

    /// Test contract code.
    static TEST_CONTRACT_CODE_HEX: &str =
        include_str!("../../../../../tests/e2e/evm/contracts/use_gas/evm_use_gas.hex");

    #[test]
    fn test_call_gas_used() {
        // Test basic.
        let ret = call_contract(
            H160([
                0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x09,
            ]),
            &ethabi::encode(&[Token::Bytes(Vec::new())]),
            10_560,
        )
        .unwrap();

        let gas_usage = ethabi::decode(&[ParamType::Uint(128)], &ret.unwrap().output)
            .expect("call should return gas usage")
            .pop()
            .unwrap()
            .into_uint()
            .expect("call should return uint")
            .try_into()
            .unwrap_or(u64::max_value());
        assert_eq!(gas_usage, 10, "call should return gas usage");

        // Test use gas in contract.
        let mut mock = Mock::default();
        let ctx = mock.create_ctx_for_runtime::<TestRuntime>(false);
        let mut signer = EvmSigner::new(0, keys::dave::sigspec());

        // Create contract.
        let contract_address = init_and_deploy_contract(&ctx, &mut signer, TEST_CONTRACT_CODE_HEX);

        let expected_gas_used = 22_659;

        // Call into the test contract.
        let dispatch_result =
            signer.call_evm(&ctx, contract_address.into(), "test_gas_used", &[], &[]);
        assert!(
            dispatch_result.result.is_success(),
            "test gas used should succeed"
        );
        assert_eq!(dispatch_result.tags.len(), 2, "2 emitted tags expected");

        // Check actual gas usage.
        let expected = cbor::to_vec(vec![Event::GasUsed {
            amount: expected_gas_used,
        }]);
        assert_eq!(
            dispatch_result.tags[0].value, expected,
            "expected events emitted"
        );
    }

    #[test]
    fn test_pad_gas() {
        // Test basic.
        call_contract(
            H160([
                0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xa,
            ]),
            &ethabi::encode(&[Token::Uint(1.into())]),
            10_560,
        )
        .expect("call should return something")
        .expect_err("call should fail as the input gas amount is to small");

        let ret = call_contract(
            H160([
                0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xa,
            ]),
            &ethabi::encode(&[Token::Uint(20.into())]),
            10_560,
        )
        .unwrap();
        assert_eq!(ret.unwrap().output.len(), 0);

        // Test out of gas.
        call_contract(
            H160([
                0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xa,
            ]),
            &ethabi::encode(&[Token::Uint(20_000.into())]),
            10_560,
        )
        .expect("call should return something")
        .expect_err("call should fail as the gas limit is reached");

        // Test gas padding.
        let mut mock = Mock::default();
        let ctx = mock.create_ctx_for_runtime::<TestRuntime>(false);
        let mut signer = EvmSigner::new(0, keys::dave::sigspec());

        // Create contract.
        let contract_address = init_and_deploy_contract(&ctx, &mut signer, TEST_CONTRACT_CODE_HEX);

        let expected_gas = 41_359;

        // Call into the test contract path for `if param > 10`.
        let dispatch_result = signer.call_evm(
            &ctx,
            contract_address.into(),
            "test_pad_gas",
            &[ParamType::Uint(128)],
            &[Token::Uint(100.into())],
        );
        assert!(
            dispatch_result.result.is_success(),
            "pad gas should succeed"
        );
        assert_eq!(dispatch_result.tags.len(), 1, "1 emitted tags expected");

        let expected = cbor::to_vec(vec![Event::GasUsed {
            amount: expected_gas,
        }]);
        assert_eq!(
            dispatch_result.tags[0].value, expected,
            "expected gas usage"
        );

        // Call into the test contract path `if param < 10`.
        let dispatch_result = signer.call_evm(
            &ctx,
            contract_address.into(),
            "test_pad_gas",
            &[ParamType::Uint(128)],
            &[Token::Uint(1.into())],
        );
        assert!(
            dispatch_result.result.is_success(),
            "pad gas should succeed"
        );
        assert_eq!(dispatch_result.tags.len(), 1, "1 emitted tags expected");

        let expected = cbor::to_vec(vec![Event::GasUsed {
            amount: expected_gas, // Gas usage should match for both code paths in the contract.
        }]);
        assert_eq!(
            dispatch_result.tags[0].value, expected,
            "expected gas usage"
        );
    }
}
