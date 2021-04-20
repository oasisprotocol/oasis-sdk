use oasis_core_runtime::common::cbor;

use crate::{
    context::Mode,
    module,
    testing::{keys, mock},
    types::{token, transaction},
};

use super::{Module as Core, API as _};

#[test]
fn test_use_gas() {
    const MAX_GAS: u64 = 1000;
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    let mut tx = mock::transaction();
    tx.auth_info.fee.gas = MAX_GAS;

    ctx.with_tx(tx.clone(), |mut tx_ctx, _call| {
        Core::use_gas(&mut tx_ctx, MAX_GAS).expect("using gas under limit should succeed");
    });

    ctx.with_tx(tx.clone(), |mut tx_ctx, _call| {
        Core::use_gas(&mut tx_ctx, MAX_GAS)
            .expect("gas across separate transactions shouldn't accumulate");
    });

    ctx.with_tx(tx.clone(), |mut tx_ctx, _call| {
        Core::use_gas(&mut tx_ctx, MAX_GAS).unwrap();
        Core::use_gas(&mut tx_ctx, 1).expect_err("gas in same transaction should accumulate");
    });

    ctx.with_tx(tx.clone(), |mut tx_ctx, _call| {
        Core::use_gas(&mut tx_ctx, 1).unwrap();
        Core::use_gas(&mut tx_ctx, u64::max_value()).expect_err("overflow should cause error");
    });
}

#[test]
fn test_query_estimate_gas() {
    const MAX_GAS: u64 = 100;
    const METHOD_WASTE_GAS: &str = "test.WasteGas";
    let mut mock = mock::Mock::default();
    mock.methods.register_callable(module::CallableMethodInfo {
        name: METHOD_WASTE_GAS,
        handler: |_mi, ctx, _args| {
            Core::use_gas(ctx, MAX_GAS).expect("use_gas should succeed");
            transaction::CallResult::Ok(cbor::Value::Null)
        },
    });
    let mut ctx = mock.create_ctx();
    ctx.mode = Mode::CheckTx;

    let tx = transaction::Transaction {
        version: 1,
        call: transaction::Call {
            method: METHOD_WASTE_GAS.to_owned(),
            body: cbor::Value::Null,
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![transaction::SignerInfo::new(keys::alice::pk(), 0)],
            fee: transaction::Fee {
                amount: token::BaseUnits::new(0.into(), token::Denomination::NATIVE),
                gas: u64::max_value(),
            },
        },
    };

    let est = Core::query_estimate_gas(&mut ctx, tx).expect("query_estimate_gas should succeed");
    assert_eq!(est, MAX_GAS, "estimated gas should be correct");
}
