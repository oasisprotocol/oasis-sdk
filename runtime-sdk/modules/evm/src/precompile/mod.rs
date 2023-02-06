//! EVM precompiles.

use std::marker::PhantomData;

use evm::{
    executor::stack::{PrecompileFailure, PrecompileHandle, PrecompileOutput, PrecompileSet},
    ExitError,
};
use primitive_types::H160;

use crate::{backend::EVMBackendExt, Config};

mod confidential;
mod standard;

#[cfg(test)]
mod test;

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
        if !self.is_precompile(address) {
            return None;
        }
        Some(match (address[0], address[19]) {
            (0, 1) => standard::call_ecrecover(handle),
            (0, 2) => standard::call_sha256(handle),
            (0, 3) => standard::call_ripemd160(handle),
            (0, 4) => standard::call_datacopy(handle),
            (0, 5) => standard::call_bigmodexp(handle),
            (1, 1) => confidential::call_random_bytes(handle, self.backend),
            (1, 2) => confidential::call_x25519_derive(handle),
            (1, 3) => confidential::call_deoxysii_seal(handle),
            (1, 4) => confidential::call_deoxysii_open(handle),
            (1, 5) => confidential::call_keypair_generate(handle),
            (1, 6) => confidential::call_sign(handle),
            (1, 7) => confidential::call_verify(handle),
            _ => return Cfg::additional_precompiles().and_then(|pc| pc.execute(handle)),
        })
    }

    fn is_precompile(&self, address: H160) -> bool {
        // All Ethereum precompiles are zero except for the last byte, which is no more than five.
        // Otherwise, when confidentiality is enabled, Oasis precompiles start with one and have a last byte of no more than four.
        let addr_bytes = address.as_bytes();
        let (first, last) = (address[0], addr_bytes[19]);
        (address[1..19].iter().all(|b| *b == 0)
            && matches!(
                (first, last, Cfg::CONFIDENTIAL),
                (0, 1..=5, _) | (1, 1..=7, true)
            ))
            || Cfg::additional_precompiles()
                .map(|pc| pc.is_precompile(address))
                .unwrap_or_default()
    }
}
