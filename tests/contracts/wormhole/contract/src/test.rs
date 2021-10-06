use ethsign::SecretKey;

use oasis_contract_sdk::{testing::MockContext, Contract};
use oasis_contract_sdk_types::{testing::addresses, ExecutionContext};

use super::*;

fn prepare_vaa(
    signer: &SecretKey,
    body: spec::VAABody,
    guardian_set_index: u32,
) -> spec::ParsedVAA {
    // Hash and sign VAA body.
    let body_ser = body.serialize();
    let mut hasher = Keccak256::new();
    hasher.update(body_ser);
    let hash = hasher.finalize().to_vec();
    // Rehash the hash
    let mut hasher = Keccak256::new();
    hasher.update(hash);
    let hash = hasher.finalize().to_vec();

    let sig = signer.sign(&hash).unwrap();
    let sig = vec![[sig.r.to_vec(), sig.s.to_vec(), [sig.v].to_vec()].concat()];

    body.into_vaa(1, guardian_set_index, sig, hash)
}

#[test]
fn test_wormhole_contract() {
    let g_secret = SecretKey::from_raw(&[1; 32]).unwrap();
    let g_public = g_secret.public();
    //let g_address = pk.address();
    let g = spec::GuardianAddress::from_bytes(g_public.address()).unwrap();

    // Create a mock execution context with default values.
    let mut ctx: MockContext = ExecutionContext::default().into();

    // Instantiate the contract.
    Wormhole::instantiate(
        &mut ctx,
        Request::Instantiate {
            params: types::InstantiateParameters {
                governance_chain: 1,
                governance_address: addresses::alice::address().into(),
                initial_guardian_set_index: 1,
                initial_guardian_set: spec::GuardianSet {
                    addresses: Vec::new(),
                    expiration_time: 0,
                },
                guardian_set_expiry: 100_000,
                fee: 10,
            },
        },
    )
    .expect_err("instantiation with empty guardian set should fail");

    Wormhole::instantiate(
        &mut ctx,
        Request::Instantiate {
            params: types::InstantiateParameters {
                governance_chain: 1,
                governance_address: addresses::alice::address().into(),
                initial_guardian_set_index: 1,
                initial_guardian_set: spec::GuardianSet {
                    addresses: vec![g.clone()],
                    expiration_time: 0,
                },
                guardian_set_expiry: 100_000,
                fee: 10,
            },
        },
    )
    .expect("instantiation set should work");

    // Queries
    let resp = Wormhole::query(&mut ctx, Request::GuardianSetInfo)
        .expect("querying guardian set info should work");
    assert_eq!(
        resp,
        Response::GuardianSetInfo {
            guardian_set: spec::GuardianSet {
                addresses: vec![g],
                expiration_time: 0,
            },
        },
    );

    let resp = Wormhole::query(&mut ctx, Request::GetConfig).expect("querying config should work");
    assert_eq!(
        resp,
        Response::GetConfig {
            config: types::Config {
                guardian_set_index: 1,
                guardian_set_expiry: 100_000,
                governance_chain: 1,
                governance_address: addresses::alice::address().into(),
                fee: token::BaseUnits::new(10, token::Denomination::NATIVE),
            },
        },
    );

    // Test Post Message.
    let message = vec![0, 1, 2, 3, 4];
    let nonce = 0;
    Wormhole::call(
        &mut ctx,
        Request::PostMessage {
            message: message.clone(),
            nonce,
        },
    )
    .expect_err("PostMessage without fee should fail");

    ctx.ec.deposited_tokens = vec![token::BaseUnits::new(5, token::Denomination::NATIVE)];
    Wormhole::call(
        &mut ctx,
        Request::PostMessage {
            message: message.clone(),
            nonce,
        },
    )
    .expect_err("PostMessage with not enough fee should fail");

    ctx.ec.deposited_tokens = vec![token::BaseUnits::new(10, token::Denomination::NATIVE)];
    Wormhole::call(&mut ctx, Request::PostMessage { message, nonce })
        .expect("PostMessage with enough fee should work");

    let body = spec::VAABody {
        timestamp: 1234,
        nonce: 1,
        emitter_chain: 1, // Governance chain.
        emitter_address: addresses::alice::address().into(),
        sequence: 1,
        consistency_level: 1,
        payload: vec![97, 98, 99, 100],
    };
    let mut vaa = prepare_vaa(&g_secret, body.clone(), 1);

    Wormhole::query(
        &mut ctx,
        Request::VerifyVAA {
            vaa: vaa.serialize(),
            block_time: 123,
        },
    )
    .expect("VerifyVAA should work");

    // Change the timestamp which invalidates the body signature.
    vaa.timestamp = 1;
    Wormhole::query(
        &mut ctx,
        Request::VerifyVAA {
            vaa: vaa.serialize(),
            block_time: 123,
        },
    )
    .expect_err("VerifyVAA should reject VAA with an invalid signature");

    // VAA with invalid guardian set index should not be valid.
    let vaa = prepare_vaa(&g_secret, body.clone(), 2);

    Wormhole::query(
        &mut ctx,
        Request::VerifyVAA {
            vaa: vaa.serialize(),
            block_time: 123,
        },
    )
    .expect_err("VerifyVAA should reject VAA with invalid guardian set index");

    // VAA not signed by the guardian should not be valid.
    let secret = SecretKey::from_raw(&[2; 32]).unwrap();
    let vaa = prepare_vaa(&secret, body.clone(), 1);

    Wormhole::query(
        &mut ctx,
        Request::VerifyVAA {
            vaa: vaa.serialize(),
            block_time: 123,
        },
    )
    .expect_err("VerifyVAA should reject VAA not signed by the guardian");

    // TODO: SubmitVAA.
}
