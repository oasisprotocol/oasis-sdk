//! EVM precompiles.

use std::marker::PhantomData;

use evm::{
    interpreter::{
        runtime::{RuntimeBackend, RuntimeState},
        ExitError, ExitSucceed,
    },
    standard::{GasometerState, PrecompileSet},
    GasMutState,
};
use oasis_runtime_sdk::Context;
use primitive_types::H160;

use crate::{engine::state::ParentGasInfo, Config};

mod confidential;
pub mod contract;
pub mod erc20;
mod gas;
mod sha2;
mod subcall;

#[cfg(any(test, feature = "test"))]
pub mod testing;

/// An error returned by an EVM precompile, together with the output.
#[derive(Clone, Debug)]
pub struct PrecompileError {
    pub error: ExitError,
    pub output: Vec<u8>,
}

impl PrecompileError {
    pub fn new(error: ExitError, output: Vec<u8>) -> Self {
        Self { error, output }
    }
}

impl From<ExitError> for PrecompileError {
    fn from(error: ExitError) -> Self {
        Self {
            error,
            output: Vec::new(),
        }
    }
}

/// A successful result returned by an EVM precompile, together with the output.
#[derive(Clone, Debug)]
pub struct PrecompileSuccess {
    pub status: ExitSucceed,
    pub output: Vec<u8>,
}

impl PrecompileSuccess {
    pub fn new(status: ExitSucceed, output: Vec<u8>) -> Self {
        Self { status, output }
    }
}

/// A result returned by an EVM precompile, either success or error.
pub type PrecompileResult = Result<PrecompileSuccess, PrecompileError>;

macro_rules! ensure_gas {
    ($math:expr) => {
        $math.ok_or(evm::interpreter::ExitError::Exception(
            evm::interpreter::ExitException::OutOfGas,
        ))?
    };
}

// ceil(bytes/32)
fn bytes_to_words(bytes: u64) -> u64 {
    bytes.saturating_add(31) / 32
}

/// Records linear gas cost: base + word*ceil(len/32)
fn record_linear_cost<G: GasMutState>(
    gasometer: &mut G,
    len: u64,
    base: u64,
    word: u64,
) -> Result<(), evm::interpreter::ExitError> {
    record_multilinear_cost(gasometer, len, 0, word, 0, base)
}

// Records a*ceil(x/32) + b*ceil(y/32) + c, or an error if out of gas.
fn record_multilinear_cost<G: GasMutState>(
    gasometer: &mut G,
    x: u64,
    y: u64,
    a: u64,
    b: u64,
    c: u64,
) -> Result<(), evm::interpreter::ExitError> {
    let cost = c;
    let ax = ensure_gas!(a.checked_mul(bytes_to_words(x)));
    let by = ensure_gas!(b.checked_mul(bytes_to_words(y)));
    let cost = ensure_gas!(cost.checked_add(ax));
    let cost = ensure_gas!(cost.checked_add(by));
    gasometer.record_gas(cost.into())?;
    Ok(())
}

pub(crate) struct Precompiles<'a, Cfg: Config, C: Context> {
    ctx: &'a C,
    config: PhantomData<Cfg>,
}

impl<'a, Cfg: Config, C: Context> Precompiles<'a, Cfg, C> {
    pub(crate) fn new(ctx: &'a C) -> Self {
        Self {
            ctx,
            config: PhantomData,
        }
    }
}

impl<Cfg, C, G, H> PrecompileSet<G, H> for Precompiles<'_, Cfg, C>
where
    Cfg: Config,
    C: Context,
    G: AsRef<RuntimeState>
        + AsRef<evm::standard::Config>
        + AsRef<GasometerState>
        + GasMutState
        + ParentGasInfo,
    H: RuntimeBackend,
{
    fn execute(
        &self,
        code_address: H160,
        input: &[u8],
        gasometer: &mut G,
        handler: &mut H,
    ) -> Option<(evm::interpreter::ExitResult, Vec<u8>)> {
        // First try the standard Ethereum precompiles.
        let std_precompiles = evm_precompile::StandardPrecompileSet;
        if let Some(result) = std_precompiles.execute(code_address, input, gasometer, handler) {
            return Some(result);
        }

        // Then try the Oasis-specific precompiles.
        let result = match (
            code_address[0],
            code_address[18],
            code_address[19],
            Cfg::CONFIDENTIAL,
        ) {
            // Oasis-specific, confidential.
            // 0x0100000000000000000000000000000000000001
            (1, 0, 1, true) => confidential::call_random_bytes(input, gasometer, self.ctx),
            // 0x0100000000000000000000000000000000000002
            (1, 0, 2, true) => confidential::call_x25519_derive(input, gasometer),
            // 0x0100000000000000000000000000000000000003
            (1, 0, 3, true) => confidential::call_deoxysii_seal(input, gasometer),
            // 0x0100000000000000000000000000000000000004
            (1, 0, 4, true) => confidential::call_deoxysii_open(input, gasometer),
            // 0x0100000000000000000000000000000000000005
            (1, 0, 5, true) => confidential::call_keypair_generate(input, gasometer),
            // 0x0100000000000000000000000000000000000006
            (1, 0, 6, true) => confidential::call_sign(input, gasometer),
            // 0x0100000000000000000000000000000000000007
            (1, 0, 7, true) => confidential::call_verify(input, gasometer),
            // 0x0100000000000000000000000000000000000008
            (1, 0, 8, true) => confidential::call_curve25519_compute_public(input, gasometer),
            // 0x0100000000000000000000000000000000000009
            (1, 0, 9, true) => gas::call_gas_used(input, gasometer),
            // 0x010000000000000000000000000000000000000a
            (1, 0, 10, true) => gas::call_pad_gas(input, gasometer),
            // Oasis-specific, general.
            // 0x0100000000000000000000000000000000000101
            (1, 1, 1, _) => sha2::call_sha512_256(input, gasometer),
            // 0x0100000000000000000000000000000000000102
            (1, 1, 2, _) => sha2::call_sha512(input, gasometer),
            // 0x0100000000000000000000000000000000000103
            (1, 1, 3, _) => subcall::call_subcall(code_address, input, gasometer, self.ctx),
            // 0x0100000000000000000000000000000000000104
            (1, 1, 4, _) => sha2::call_sha384(input, gasometer),
            _ => {
                // If nothing matches, try additional precompiles.
                return Cfg::additional_precompiles()
                    .and_then(|pc| pc.execute(code_address, input, gasometer, handler));
            }
        };

        match result {
            Ok((succeed, retval)) => Some((succeed.into(), retval)),
            Err(err) => Some((err.into(), Vec::new())),
        }
    }
}
