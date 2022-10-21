//! EVM precompiles.

use std::marker::PhantomData;

use evm::{
    executor::stack::{PrecompileFailure, PrecompileOutput, PrecompileSet},
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

/// Linear gas cost: base + word*ceil(len/32)
fn linear_cost(
    target_gas: Option<u64>,
    len: u64,
    base: u64,
    word: u64,
) -> Result<u64, PrecompileFailure> {
    multilinear_cost(target_gas, len, 0, word, 0, base)
}

// Returns a*ceil(x/32) + b*ceil(y/32) + c, or an error if out of gas.
fn multilinear_cost(
    target_gas: Option<u64>,
    x: u64,
    y: u64,
    a: u64,
    b: u64,
    c: u64,
) -> Result<u64, PrecompileFailure> {
    let cost = c;
    let ax = ensure_gas!(a.checked_mul(bytes_to_words(x)));
    let by = ensure_gas!(b.checked_mul(bytes_to_words(y)));
    let cost = ensure_gas!(cost.checked_add(ax));
    let cost = ensure_gas!(cost.checked_add(by));
    if let Some(target_gas) = target_gas {
        if cost > target_gas {
            return Err(PrecompileFailure::Error {
                exit_status: ExitError::OutOfGas,
            });
        }
    }
    Ok(cost)
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
    fn execute(
        &self,
        address: H160,
        input: &[u8],
        gas_limit: Option<u64>,
        context: &evm::Context,
        is_static: bool,
    ) -> Option<PrecompileResult> {
        if !self.is_precompile(address) {
            return None;
        }
        Some(match (address[0], address[19]) {
            (0, 1) => standard::call_ecrecover(input, gas_limit, context, is_static),
            (0, 2) => standard::call_sha256(input, gas_limit, context, is_static),
            (0, 3) => standard::call_ripemd160(input, gas_limit, context, is_static),
            (0, 4) => standard::call_datacopy(input, gas_limit, context, is_static),
            (0, 5) => standard::call_bigmodexp(input, gas_limit, context, is_static),
            (1, 1) => {
                confidential::call_random_bytes(input, gas_limit, context, is_static, self.backend)
            }
            (1, 2) => confidential::call_x25519_derive(input, gas_limit, context, is_static),
            (1, 3) => confidential::call_deoxysii_seal(input, gas_limit, context, is_static),
            (1, 4) => confidential::call_deoxysii_open(input, gas_limit, context, is_static),
            (1, 5) => confidential::call_keypair_generate(input, gas_limit, context, is_static),
            (1, 6) => confidential::call_sign(input, gas_limit, context, is_static),
            (1, 7) => confidential::call_verify(input, gas_limit, context, is_static),
            _ => {
                return Cfg::additional_precompiles()
                    .and_then(|pc| pc.execute(address, input, gas_limit, context, is_static))
            }
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
