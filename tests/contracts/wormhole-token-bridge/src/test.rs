use oasis_contract_sdk::{testing::MockContext, Contract};
use oasis_contract_sdk_types::{testing::addresses, CodeId, ExecutionContext};

use super::*;

#[test]
fn test_instantiate() {
    // Create a mock execution context with default values.
    let mut ctx: MockContext = ExecutionContext::default().into();

    // Instantiate the contract.
    WormholeTokenBridge::instantiate(
        &mut ctx,
        Request::Instantiate(types::Configuration {
            governance_chain: 1,
            governance_address: addresses::alice::address().into(),
            wormhole_contract: InstanceId::from(1),
            wrapped_asset_code_id: CodeId::from(1),
        }),
    )
    .expect("instantiation should work");
}

#[test]
fn test_create_asset_meta() {
    // Create a mock execution context with default values.
    let mut ctx: MockContext = ExecutionContext::default().into();

    // Instantiate the contract.
    WormholeTokenBridge::instantiate(
        &mut ctx,
        Request::Instantiate(types::Configuration {
            governance_chain: 1,
            governance_address: addresses::alice::address().into(),
            wormhole_contract: InstanceId::from(1),
            wrapped_asset_code_id: CodeId::from(1),
        }),
    )
    .expect("instantiation should work");

    // Instantiate the contract.
    WormholeTokenBridge::call(
        &mut ctx,
        Request::CreateAssetMeta {
            asset_instance_id: InstanceId::from(1),
            nonce: 1,
        },
    )
    .expect("create asset meta should work");

    // Ensure message was emitted.
    assert_eq!(ctx.messages.len(), 1, "message should be emitted");

    // Test handle reply.
    match ctx.messages.get(0).unwrap().clone() {
        Message::Call { id, data, .. } => {
            let response = oas20::Response::TokenInformation {
                token_information: oas20::TokenInformation {
                    name: "TEST".to_string(),
                    symbol: "TST".to_string(),
                    decimals: 5,
                    total_supply: 100_000,
                    minting: None,
                },
            };
            WormholeTokenBridge::handle_reply(
                &mut ctx,
                Reply::Call {
                    id,
                    result: CallResult::Ok(cbor::to_value(response)),
                    data,
                },
            )
            .expect("create asset meta token query reply should work");

            // Ensure another message was emitted.
            assert_eq!(ctx.messages.len(), 2, "message should be emitted");
        }

        _ => panic!("unexpected message"),
    }
}

#[test]
fn test_initiate_transfer() {
    // Create a mock execution context with default values.
    let mut ctx: MockContext = ExecutionContext::default().into();

    // Instantiate the contract.
    WormholeTokenBridge::instantiate(
        &mut ctx,
        Request::Instantiate(types::Configuration {
            governance_chain: 1,
            governance_address: addresses::alice::address().into(),
            wormhole_contract: InstanceId::from(1),
            wrapped_asset_code_id: CodeId::from(1),
        }),
    )
    .expect("instantiation should work");

    WormholeTokenBridge::call(
        &mut ctx,
        Request::InitiateTransfer {
            asset: InstanceId::from(1),
            amount: 0, // Should fail.
            recipient_chain: 1,
            recipient: addresses::bob::address().into(),
            fee: 0,
            nonce: 0,
        },
    )
    .expect_err("initiate transfer with invalid amount should fail");

    WormholeTokenBridge::call(
        &mut ctx,
        Request::InitiateTransfer {
            asset: InstanceId::from(1),
            amount: 10,
            recipient_chain: wormhole::spec::OASIS_CHAIN_ID, // Should fail.
            recipient: addresses::bob::address().into(),
            fee: 0,
            nonce: 0,
        },
    )
    .expect_err("initiate transfer with oasis recipient chain should fail");

    WormholeTokenBridge::call(
        &mut ctx,
        Request::InitiateTransfer {
            asset: InstanceId::from(1),
            amount: 10,
            recipient_chain: 1,
            recipient: addresses::bob::address().into(),
            fee: 20,
            nonce: 0,
        },
    )
    .expect_err("initiate fee greater than amount should fail");

    WormholeTokenBridge::call(
        &mut ctx,
        Request::InitiateTransfer {
            asset: InstanceId::from(1),
            amount: 109,
            recipient_chain: 1,
            recipient: addresses::bob::address().into(),
            fee: 20,
            nonce: 0,
        },
    )
    .expect("initiate transfer should work");

    // Ensure message was emitted.
    assert_eq!(ctx.messages.len(), 1, "message should be emitted");
}
