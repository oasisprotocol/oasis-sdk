use revm::precompile::{Address, Precompile, PrecompileWithAddress};

mod confidential;
mod gas;
mod sha2;

/// Helper to generate the precompile address for Oasis-specific precompiles.
#[inline]
pub const fn oasis_addr(a18: u8, a19: u8) -> Address {
    Address::new([
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, a18, a19,
    ])
}

pub fn new() -> Vec<PrecompileWithAddress> {
    vec![
        // Oasis-specific, confidential.
        // TODO: random_bytes
        /*PrecompileWithAddress(
            oasis_addr(0, 1),
            Precompile::Env(confidential::call_random_bytes),
        ),*/
        PrecompileWithAddress(
            oasis_addr(0, 2),
            Precompile::Standard(confidential::call_x25519_derive),
        ),
        PrecompileWithAddress(
            oasis_addr(0, 3),
            Precompile::Standard(confidential::call_deoxysii_seal),
        ),
        PrecompileWithAddress(
            oasis_addr(0, 4),
            Precompile::Standard(confidential::call_deoxysii_open),
        ),
        PrecompileWithAddress(
            oasis_addr(0, 5),
            Precompile::Standard(confidential::call_keypair_generate),
        ),
        PrecompileWithAddress(
            oasis_addr(0, 6),
            Precompile::Standard(confidential::call_sign),
        ),
        PrecompileWithAddress(
            oasis_addr(0, 7),
            Precompile::Standard(confidential::call_verify),
        ),
        PrecompileWithAddress(
            oasis_addr(0, 8),
            Precompile::Standard(confidential::call_curve25519_compute_public),
        ),
        PrecompileWithAddress(oasis_addr(0, 9), Precompile::Env(gas::call_gas_used)),
        PrecompileWithAddress(oasis_addr(0, 10), Precompile::Env(gas::call_pad_gas)),
        // Oasis-specific, general.
        PrecompileWithAddress(
            oasis_addr(1, 1),
            Precompile::Standard(sha2::call_sha512_256),
        ),
        PrecompileWithAddress(oasis_addr(1, 2), Precompile::Standard(sha2::call_sha512)),
        // TODO: subcall
        /*PrecompileWithAddress(
            oasis_addr(1, 3),
            Precompile::Env(subcall::call_subcall),
        ),*/
        PrecompileWithAddress(oasis_addr(1, 4), Precompile::Standard(sha2::call_sha384)),
    ]
}
