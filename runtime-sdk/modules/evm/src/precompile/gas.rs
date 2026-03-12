use evm::{
    interpreter::{ExitError, ExitException, ExitSucceed},
    standard::GasometerState,
    GasMutState,
};

use crate::engine::state::ParentGasInfo;

const GAS_USED_COST: u64 = 10;
const PAD_GAS_COST: u64 = 10;

pub(super) fn call_gas_used<G>(
    _input: &[u8],
    gasometer: &mut G,
) -> Result<(ExitSucceed, Vec<u8>), ExitError>
where
    G: GasMutState + AsRef<GasometerState> + ParentGasInfo,
{
    gasometer.record_gas(GAS_USED_COST.into())?;

    let used_gas: u64 = total_used_gas(gasometer);
    let output = solabi::encode(&(used_gas,));

    Ok((ExitSucceed::Returned, output))
}

pub(super) fn call_pad_gas<G>(
    input: &[u8],
    gasometer: &mut G,
) -> Result<(ExitSucceed, Vec<u8>), ExitError>
where
    G: GasMutState + AsRef<GasometerState> + ParentGasInfo,
{
    gasometer.record_gas(PAD_GAS_COST.into())?;

    // Decode args.
    let gas_amount_big: u128 = solabi::decode(input)
        .map_err(|e| ExitError::Exception(ExitException::Other(e.to_string().into())))?;
    let gas_amount = gas_amount_big.try_into().unwrap_or(u64::MAX);

    let used_gas: u64 = total_used_gas(gasometer);

    // Fail if more gas than the desired padding was already used.
    if gas_amount < used_gas {
        return Err(ExitError::Exception(ExitException::Other(
            "gas pad amount less than already used gas"
                .to_string()
                .into(),
        )));
    }

    // Record the remainder so that the gas use is padded to the desired amount.
    gasometer.record_gas((gas_amount - used_gas).into())?;

    Ok((ExitSucceed::Returned, Vec::new()))
}

fn total_used_gas<G>(gasometer: &G) -> u64
where
    G: AsRef<GasometerState> + ParentGasInfo,
{
    let gs: &GasometerState = gasometer.as_ref();
    let frame_used: u64 = gs.total_used_gas();
    let parent_used: u64 = gasometer.parent_used_gas().try_into().unwrap_or(u64::MAX);
    parent_used.saturating_add(frame_used)
}

#[cfg(test)]
mod test {
    use super::super::testing::*;
    use crate::{
        mock::EvmSigner,
        precompile::testing::{init_and_deploy_contract, TestRuntime},
    };
    use oasis_runtime_sdk::{
        modules::core::Event,
        testing::{keys, mock::Mock},
    };

    /// Test contract code.
    static TEST_CONTRACT_CODE_HEX: &str =
        include_str!("../../../../../tests/e2e/evm/contracts/use_gas/evm_use_gas.hex");

    #[test]
    fn test_call_gas_used_basic() {
        // Test basic.
        let ret = call_contract(
            H160([
                0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x09,
            ]),
            &solabi::encode(&(solabi::Bytes(Vec::new()),)),
            30_000,
        )
        .unwrap();

        let expected_gas_usage: u64 =
            21_000 /* base call transaction cost */ +
            4*63 /* zero data cost */ +
            16 /* non-zero data cost */ +
            10 /* precompile cost */;
        let gas_usage_big: u128 = solabi::decode(&ret).expect("call should return gas usage");
        let gas_usage: u64 = gas_usage_big.try_into().unwrap_or(u64::max_value());
        assert_eq!(
            gas_usage, expected_gas_usage,
            "call should return gas usage"
        );
    }

    #[test]
    fn test_call_gas_used_contract() {
        // Test use gas in contract.
        let mut mock = Mock::default();
        let ctx = mock.create_ctx_for_runtime::<TestRuntime>(false);
        let mut signer = EvmSigner::new(0, keys::dave::sigspec());

        // Create contract.
        let contract_address = init_and_deploy_contract(&ctx, &mut signer, TEST_CONTRACT_CODE_HEX);

        let expected_gas_used = 25_164;

        // Call into the test contract.
        let dispatch_result = signer.call_evm(
            &ctx,
            contract_address.into(),
            solabi::selector!("test_gas_used()"),
            &(),
        );
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
            &solabi::encode(&(1_u64,)),
            40_000,
        )
        .expect_err("call should fail as the input gas amount is to small");

        let ret = call_contract(
            H160([
                0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xa,
            ]),
            &solabi::encode(&(30_000u64,)),
            40_000,
        )
        .unwrap();
        assert_eq!(ret.len(), 0);

        // Test out of gas.
        call_contract(
            H160([
                0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xa,
            ]),
            &solabi::encode(&(50_000_u64,)),
            40_000,
        )
        .expect_err("call should fail as the gas limit is reached");

        // Test gas padding.
        let mut mock = Mock::default();
        let ctx = mock.create_ctx_for_runtime::<TestRuntime>(false);
        let mut signer = EvmSigner::new(0, keys::dave::sigspec());

        // Create contract.
        let contract_address = init_and_deploy_contract(&ctx, &mut signer, TEST_CONTRACT_CODE_HEX);

        let expected_gas = 50_156;

        // Call into the test contract path for `if param > 10`.
        let dispatch_result = signer.call_evm(
            &ctx,
            contract_address.into(),
            solabi::selector!("test_pad_gas(uint128)"),
            &(100_u128,),
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
            solabi::selector!("test_pad_gas(uint128)"),
            &(1_u128,),
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
