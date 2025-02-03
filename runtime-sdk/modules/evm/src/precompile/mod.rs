//! EVM precompiles.

use std::{cmp::min, marker::PhantomData};

use evm::{
    executor::stack::{
        IsPrecompileResult, PrecompileFailure, PrecompileHandle, PrecompileOutput, PrecompileSet,
    },
    ExitError,
};
use primitive_types::H160;

use crate::{backend::EVMBackendExt, Config};

mod confidential;
pub mod contract;
pub mod erc20;
mod gas;
mod sha2;
mod standard;
mod subcall;

#[cfg(any(test, feature = "test"))]
pub mod testing;

// Some types matching evm::executor::stack.
type PrecompileResult = Result<PrecompileOutput, PrecompileFailure>;

macro_rules! ensure_gas {
    ($math:expr) => {
        $math.ok_or(PrecompileFailure::Error {
            exit_status: ExitError::OutOfGas,
        })?
    };
}

// ceil(bytes/32)
fn bytes_to_words(bytes: u64) -> u64 {
    bytes.saturating_add(31) / 32
}

/// Records linear gas cost: base + word*ceil(len/32)
fn record_linear_cost(
    handle: &mut impl PrecompileHandle,
    len: u64,
    base: u64,
    word: u64,
) -> Result<(), PrecompileFailure> {
    record_multilinear_cost(handle, len, 0, word, 0, base)
}

// Records a*ceil(x/32) + b*ceil(y/32) + c, or an error if out of gas.
fn record_multilinear_cost(
    handle: &mut impl PrecompileHandle,
    x: u64,
    y: u64,
    a: u64,
    b: u64,
    c: u64,
) -> Result<(), PrecompileFailure> {
    let cost = c;
    let ax = ensure_gas!(a.checked_mul(bytes_to_words(x)));
    let by = ensure_gas!(b.checked_mul(bytes_to_words(y)));
    let cost = ensure_gas!(cost.checked_add(ax));
    let cost = ensure_gas!(cost.checked_add(by));
    handle.record_cost(cost)?;
    Ok(())
}

/// Copies bytes from source to target.
fn read_input(source: &[u8], target: &mut [u8], offset: usize) {
    if source.len() <= offset {
        return;
    }

    let len = min(target.len(), source.len() - offset);
    target[..len].copy_from_slice(&source[offset..offset + len]);
}

pub(crate) struct Precompiles<'a, Cfg: Config, B: EVMBackendExt> {
    backend: &'a B,
    config: PhantomData<Cfg>,
}

impl<'a, Cfg: Config, B: EVMBackendExt> Precompiles<'a, Cfg, B> {
    pub(crate) fn new(backend: &'a B) -> Self {
        Self {
            backend,
            config: PhantomData,
        }
    }
}

impl<Cfg: Config, B: EVMBackendExt> PrecompileSet for Precompiles<'_, Cfg, B> {
    fn execute(&self, handle: &mut impl PrecompileHandle) -> Option<PrecompileResult> {
        let address = handle.code_address();
        match self.is_precompile(address, handle.remaining_gas()) {
            IsPrecompileResult::Answer {
                is_precompile: true,
                extra_cost,
            } => {
                if let Err(e) = handle.record_cost(extra_cost) {
                    return Some(Err(e.into()));
                }
            }
            IsPrecompileResult::OutOfGas => {
                return Some(Err(ExitError::OutOfGas.into()));
            }
            _ => {
                return None;
            }
        }
        Some(match (address[0], address[18], address[19]) {
            // Ethereum-compatible.
            (0, 0, 1) => standard::call_ecrecover(handle),
            (0, 0, 2) => standard::call_sha256(handle),
            (0, 0, 3) => standard::call_ripemd160(handle),
            (0, 0, 4) => standard::call_datacopy(handle),
            (0, 0, 5) => standard::call_bigmodexp(handle),
            (0, 0, 6) => standard::call_bn128_add(handle),
            (0, 0, 7) => standard::call_bn128_mul(handle),
            (0, 0, 8) => standard::call_bn128_pairing(handle),
            // Oasis-specific, confidential.
            (1, 0, 1) => confidential::call_random_bytes(handle, self.backend),
            (1, 0, 2) => confidential::call_x25519_derive(handle),
            (1, 0, 3) => confidential::call_deoxysii_seal(handle),
            (1, 0, 4) => confidential::call_deoxysii_open(handle),
            (1, 0, 5) => confidential::call_keypair_generate(handle),
            (1, 0, 6) => confidential::call_sign(handle),
            (1, 0, 7) => confidential::call_verify(handle),
            (1, 0, 8) => confidential::call_curve25519_compute_public(handle),
            (1, 0, 9) => gas::call_gas_used(handle),
            (1, 0, 10) => gas::call_pad_gas(handle),
            // Oasis-specific, general.
            (1, 1, 1) => sha2::call_sha512_256(handle),
            (1, 1, 2) => sha2::call_sha512(handle),
            (1, 1, 3) => subcall::call_subcall(handle, self.backend),
            (1, 1, 4) => sha2::call_sha384(handle),
            _ => return Cfg::additional_precompiles().and_then(|pc| pc.execute(handle)),
        })
    }

    fn is_precompile(&self, address: H160, remaining_gas: u64) -> IsPrecompileResult {
        // See above table in `execute` for matching on what is a valid precompile address.
        let addr_bytes = address.as_bytes();
        let (a0, a18, a19) = (address[0], addr_bytes[18], addr_bytes[19]);
        if address[1..18].iter().all(|b| *b == 0)
            && matches!(
                (a0, a18, a19, Cfg::CONFIDENTIAL),
                // Ethereum-compatible.
                (0, 0, 1..=8, _) |
                // Oasis-specific, confidential.
                (1, 0, 1..=10, true) |
                // Oasis-specific, general.
                (1, 1, 1..=4, _)
            )
        {
            IsPrecompileResult::Answer {
                is_precompile: true,
                extra_cost: 0,
            }
        } else {
            Cfg::additional_precompiles()
                .map(|pc| pc.is_precompile(address, remaining_gas))
                .unwrap_or(IsPrecompileResult::Answer {
                    is_precompile: false,
                    extra_cost: 0,
                })
        }
    }
}
