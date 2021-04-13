use crate::testing::mock;

use super::{Module as Core, API as _};

#[test]
fn test_use_gas() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    let mut tx = mock::transaction();
    tx.auth_info.fee.gas = 1000;

    ctx.with_tx(tx.clone(), |mut tx_ctx, _call| {
        Core::use_gas(&mut tx_ctx, 100)
            .expect("using gas under limit should succeed");
    });

    ctx.with_tx(tx.clone(), |mut tx_ctx, _call| {
        Core::use_gas(&mut tx_ctx, 950)
            .expect("gas across separate transactions shouldn't accumulate");
    });

    ctx.with_tx(tx.clone(), |mut tx_ctx, _call| {
        Core::use_gas(&mut tx_ctx, 600).unwrap();
        Core::use_gas(&mut tx_ctx, 600)
            .expect_err("gas in same transaction should accumulate");
    });

    ctx.with_tx(tx.clone(), |mut tx_ctx, _call| {
        Core::use_gas(&mut tx_ctx, 1).unwrap();
        Core::use_gas(&mut tx_ctx, u64::max_value()).expect_err("overflow should cause error");
    });
}
