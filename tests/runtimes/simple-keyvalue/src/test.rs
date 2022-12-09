use std::collections::BTreeMap;

use oasis_runtime_sdk::{
    module::TransactionHandler as _,
    modules::core,
    testing::mock,
    types::{token, transaction},
    Context as _, Module as _,
};

#[test]
fn test_impl_for_tuple() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    <super::Runtime as oasis_runtime_sdk::Runtime>::Core::set_params(
        ctx.runtime_state(),
        core::Parameters {
            max_batch_gas: u64::MAX,
            max_tx_size: 32 * 1024,
            max_tx_signers: 1,
            max_multisig_signers: 1,
            gas_costs: Default::default(),
            min_gas_price: {
                let mut mgp = BTreeMap::new();
                mgp.insert(token::Denomination::NATIVE, 0);
                mgp
            },
        },
    );
    let dummy_bytes = b"you look, you die".to_vec();
    <super::Runtime as oasis_runtime_sdk::Runtime>::Modules::approve_unverified_tx(
        &mut ctx,
        &transaction::UnverifiedTransaction(
            dummy_bytes.clone(),
            vec![
                transaction::AuthProof::Signature(dummy_bytes.clone().into()),
                transaction::AuthProof::Signature(dummy_bytes.into()),
            ],
        ),
    )
    .expect_err("too many authentication slots");
}
