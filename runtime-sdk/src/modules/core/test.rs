use crate::testing::mock;

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
