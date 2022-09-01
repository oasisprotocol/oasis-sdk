//! EVM precompiles.
use std::collections::BTreeMap;

use evm::{
    executor::stack::{PrecompileFailure, PrecompileFn, PrecompileOutput},
    ExitError,
};
use once_cell::sync::Lazy;
use primitive_types::H160;

mod confidential;
mod standard;

#[cfg(test)]
mod test;

// Some types matching evm::executor::stack.
type PrecompileResult = Result<PrecompileOutput, PrecompileFailure>;

/// Address of ECDSA public key recovery function.
const PRECOMPILE_ECRECOVER: H160 = H160([
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01,
]);
/// Address of of SHA2-256 hash function.
const PRECOMPILE_SHA256: H160 = H160([
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02,
]);
/// Address of RIPEMD-160 hash functions.
const PRECOMPILE_RIPEMD160: H160 = H160([
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x03,
]);
/// Address of identity which defines the output as the input.
const PRECOMPILE_DATACOPY: H160 = H160([
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04,
]);
/// Big integer modular exponentiation in EIP-198.
const PRECOMPILE_BIGMODEXP: H160 = H160([
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x05,
]);
/// Derive symmetric key from public/private key pair for X25519.
const PRECOMPILE_X25519_DERIVE: H160 = H160([
    0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02,
]);
/// Encrypt and authenticate plaintext and authenticate additional data with DeoxysII.
const PRECOMPILE_DEOXYSII_SEAL: H160 = H160([
    0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x03,
]);
/// Decrypt and authenticate plaintext and authenticate additional data with DeoxysII.
const PRECOMPILE_DEOXYSII_OPEN: H160 = H160([
    0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04,
]);

/// A set of precompiles.
pub static PRECOMPILED_CONTRACT: Lazy<BTreeMap<H160, PrecompileFn>> = Lazy::new(|| {
    BTreeMap::from([
        (
            PRECOMPILE_ECRECOVER,
            standard::call_ecrecover as PrecompileFn,
        ),
        (PRECOMPILE_SHA256, standard::call_sha256),
        (PRECOMPILE_RIPEMD160, standard::call_ripemd160),
        (PRECOMPILE_DATACOPY, standard::call_datacopy),
        (PRECOMPILE_BIGMODEXP, standard::call_bigmodexp),
        (PRECOMPILE_X25519_DERIVE, confidential::call_x25519_derive),
        (PRECOMPILE_DEOXYSII_SEAL, confidential::call_deoxysii_seal),
        (PRECOMPILE_DEOXYSII_OPEN, confidential::call_deoxysii_open),
    ])
});

/// Linear gas cost
fn linear_cost(
    target_gas: Option<u64>,
    len: u64,
    base: u64,
    word: u64,
) -> Result<u64, PrecompileFailure> {
    let cost = base
        .checked_add(word.checked_mul(len.saturating_add(31) / 32).ok_or(
            PrecompileFailure::Error {
                exit_status: ExitError::OutOfGas,
            },
        )?)
        .ok_or(PrecompileFailure::Error {
            exit_status: ExitError::OutOfGas,
        })?;

    if let Some(target_gas) = target_gas {
        if cost > target_gas {
            return Err(PrecompileFailure::Error {
                exit_status: ExitError::OutOfGas,
            });
        }
    }

    Ok(cost)
}
