//! Mock functionality for use during testing.
use base64::prelude::*;
use uint::hex::FromHex;

use oasis_runtime_sdk::{
    callformat,
    core::common::crypto::mrae::deoxysii,
    dispatcher,
    error::RuntimeError,
    module,
    testing::mock::{CallOptions, Signer},
    types::{address::SignatureAddressSpec, transaction},
    Context,
};

use crate::{
    derive_caller,
    types::{self, H160},
};

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
        ctx: &C,
        address: H160,
        name: &str,
        param_types: &[ethabi::ParamType],
        params: &[ethabi::Token],
    ) -> dispatcher::DispatchResult
    where
        C: Context,
    {
        self.call_evm_opts(ctx, address, name, param_types, params, Default::default())
    }

    /// Dispatch a call to the given EVM contract method with the given options.
    pub fn call_evm_opts<C>(
        &mut self,
        ctx: &C,
        address: H160,
        name: &str,
        param_types: &[ethabi::ParamType],
        params: &[ethabi::Token],
        opts: CallOptions,
    ) -> dispatcher::DispatchResult
    where
        C: Context,
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

    /// Ethereum address for this signer.
    pub fn address(&self) -> H160 {
        derive_caller::from_sigspec(self.sigspec()).expect("caller should be evm-compatible")
    }

    /// Dispatch a query to the given EVM contract method.
    pub fn query_evm_call<C>(
        &self,
        ctx: &C,
        address: H160,
        name: &str,
        param_types: &[ethabi::ParamType],
        params: &[ethabi::Token],
    ) -> Result<Vec<u8>, RuntimeError>
    where
        C: Context,
    {
        self.query_evm_call_opts(ctx, address, name, param_types, params, Default::default())
    }

    /// Dispatch a query to the given EVM contract method.
    pub fn query_evm_call_opts<C>(
        &self,
        ctx: &C,
        address: H160,
        name: &str,
        param_types: &[ethabi::ParamType],
        params: &[ethabi::Token],
        opts: QueryOptions,
    ) -> Result<Vec<u8>, RuntimeError>
    where
        C: Context,
    {
        let data = [
            ethabi::short_signature(name, param_types).to_vec(),
            ethabi::encode(params),
        ]
        .concat();

        self.query_evm_opts(ctx, Some(address), data, opts)
    }

    /// Dispatch a query to simulate EVM contract creation.
    pub fn query_evm_create<C>(&self, ctx: &C, init_code: Vec<u8>) -> Result<Vec<u8>, RuntimeError>
    where
        C: Context,
    {
        self.query_evm_opts(ctx, None, init_code, Default::default())
    }

    /// Dispatch a query to simulate EVM contract creation.
    pub fn query_evm_create_opts<C>(
        &self,
        ctx: &C,
        init_code: Vec<u8>,
        opts: QueryOptions,
    ) -> Result<Vec<u8>, RuntimeError>
    where
        C: Context,
    {
        self.query_evm_opts(ctx, None, init_code, opts)
    }

    /// Dispatch a query to the EVM.
    pub fn query_evm_opts<C>(
        &self,
        ctx: &C,
        address: Option<H160>,
        mut data: Vec<u8>,
        opts: QueryOptions,
    ) -> Result<Vec<u8>, RuntimeError>
    where
        C: Context,
    {
        // Handle optional encryption.
        let client_keypair = deoxysii::generate_key_pair();
        if opts.encrypt {
            data = cbor::to_vec(
                callformat::encode_call(
                    ctx,
                    transaction::Call {
                        format: transaction::CallFormat::EncryptedX25519DeoxysII,
                        method: "".into(),
                        body: cbor::Value::from(data),
                        ..Default::default()
                    },
                    &client_keypair,
                )
                .unwrap(),
            );
        }

        let mut result: Vec<u8> = self.query(
            ctx,
            "evm.SimulateCall",
            types::SimulateCallQuery {
                gas_price: 0.into(),
                gas_limit: opts.gas_limit,
                caller: opts.caller.unwrap_or_else(|| self.address()),
                address,
                value: 0.into(),
                data,
            },
        )?;

        // Handle optional decryption.
        if opts.encrypt {
            let call_result: transaction::CallResult =
                cbor::from_slice(&result).expect("result from EVM should be properly encoded");
            let call_result = callformat::decode_result(
                ctx,
                transaction::CallFormat::EncryptedX25519DeoxysII,
                call_result,
                &client_keypair,
            )
            .expect("callformat decoding should succeed");

            result = match call_result {
                module::CallResult::Ok(v) => {
                    cbor::from_value(v).expect("result from EVM should be correct")
                }
                module::CallResult::Failed {
                    module,
                    code,
                    message,
                } => return Err(RuntimeError::new(&module, code, &message)),
                module::CallResult::Aborted(e) => panic!("aborted with error: {e}"),
            };
        }

        Ok(result)
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

/// Options for making queries.
pub struct QueryOptions {
    /// Whether the call should be encrypted.
    pub encrypt: bool,
    /// Gas limit.
    pub gas_limit: u64,
    /// Use specified caller instead of signer.
    pub caller: Option<H160>,
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self {
            encrypt: false,
            gas_limit: 10_000_000,
            caller: None,
        }
    }
}

/// Load contract bytecode from a hex-encoded string.
pub fn load_contract_bytecode(raw: &str) -> Vec<u8> {
    Vec::from_hex(raw.split_whitespace().collect::<String>())
        .expect("compiled contract should be a valid hex string")
}

/// Decode a basic revert reason.
pub fn decode_reverted(msg: &str) -> Option<String> {
    decode_reverted_abi(
        msg,
        ethabi::AbiError {
            name: "Error".to_string(),
            inputs: vec![ethabi::Param {
                name: "message".to_string(),
                kind: ethabi::ParamType::String,
                internal_type: None,
            }],
        },
    )?
    .pop()
    .unwrap()
    .into_string()
}

/// Decode a revert reason accoording to the given API.
pub fn decode_reverted_abi(msg: &str, abi: ethabi::AbiError) -> Option<Vec<ethabi::Token>> {
    let raw = decode_reverted_raw(msg)?;

    // Strip (and validate) error signature.
    let signature = abi.signature();
    let raw = raw.strip_prefix(&signature.as_bytes()[..4])?;

    Some(abi.decode(raw).unwrap())
}

/// Decode a base64-encoded revert reason.
pub fn decode_reverted_raw(msg: &str) -> Option<Vec<u8>> {
    // Trim the optional reverted prefix.
    let msg = msg.trim_start_matches("reverted: ");

    BASE64_STANDARD.decode(msg).ok()
}
