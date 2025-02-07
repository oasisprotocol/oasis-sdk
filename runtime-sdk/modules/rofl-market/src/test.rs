use super::*;

#[test]
fn test_provider_stake_pool_address() {
    // Make sure the provider stake pool address doesn't change.
    assert_eq!(
        ADDRESS_PROVIDER_STAKE_POOL.to_bech32(),
        "oasis1qzta0kk6vy0yrwgllual4ntnjay68lp7vq5fs8jy"
    );
}
