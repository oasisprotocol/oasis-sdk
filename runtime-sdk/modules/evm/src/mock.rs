//! Mock functionality for use during testing.
use uint::hex::FromHex;

use oasis_runtime_sdk::{
    dispatcher,
    testing::mock::{CallOptions, Signer},
    types::address::SignatureAddressSpec,
    BatchContext,
};

use crate::types::{self, H160};

/// A mock EVM signer for use during tests.
pub struct EvmSigner(Signer);

impl EvmSigner {
    /// Create a new mock signer using the given nonce and signature spec.
    pub fn new(nonce: u64, sigspec: SignatureAddressSpec) -> Self {
        Self(Signer::new(nonce, sigspec))
    }

    /// Dispatch a call to the given EVM contract method.
    pub fn call_evm<C>(
        &mut self,
        ctx: &mut C,
        address: H160,
        name: &str,
        param_types: &[ethabi::ParamType],
        params: &[ethabi::Token],
    ) -> dispatcher::DispatchResult
    where
        C: BatchContext,
    {
        self.call_evm_opts(ctx, address, name, param_types, params, Default::default())
    }

    /// Dispatch a call to the given EVM contract method with the given options.
    pub fn call_evm_opts<C>(
        &mut self,
        ctx: &mut C,
        address: H160,
        name: &str,
        param_types: &[ethabi::ParamType],
        params: &[ethabi::Token],
        opts: CallOptions,
    ) -> dispatcher::DispatchResult
    where
        C: BatchContext,
    {
        let data = [
            ethabi::short_signature(name, param_types).to_vec(),
            ethabi::encode(params),
        ]
        .concat();

        self.call_opts(
            ctx,
            "evm.Call",
            types::Call {
                address,
                value: 0.into(),
                data,
            },
            opts,
        )
    }
}

impl std::ops::Deref for EvmSigner {
    type Target = Signer;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for EvmSigner {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Load contract bytecode from a hex-encoded string.
pub fn load_contract_bytecode(raw: &str) -> Vec<u8> {
    Vec::from_hex(raw.split_whitespace().collect::<String>())
        .expect("compiled contract should be a valid hex string")
}
