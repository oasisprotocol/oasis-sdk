//! Transaction types.
use serde::{Deserialize, Serialize};
use thiserror::Error;

use oasis_core_runtime::common::cbor;

use crate::{
    crypto::{
        multisig,
        signature::{self, PublicKey, Signature},
    },
    types::{address::Address, token},
};

/// Transaction signature domain separation context base.
pub const SIGNATURE_CONTEXT_BASE: &[u8] = b"oasis-runtime-sdk/tx: v0";
/// The latest transaction format version.
pub const LATEST_TRANSACTION_VERSION: u16 = 1;

/// Error.
#[derive(Error, PartialEq, Debug)]
pub enum Error {
    #[error("unsupported version")]
    UnsupportedVersion,
    #[error("malformed transaction")]
    MalformedTransaction,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AuthProof {
    #[serde(rename = "signature")]
    Signature(Signature),
    #[serde(rename = "multisig")]
    Multisig(multisig::SignatureSetOwned),
}

/// An unverified signed transaction.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnverifiedTransaction(#[serde(with = "serde_bytes")] Vec<u8>, pub Vec<AuthProof>);

impl UnverifiedTransaction {
    /// Verify and deserialize the unverified transaction.
    pub fn verify(self) -> Result<Transaction, Error> {
        // Deserialize the inner body.
        let body: Transaction =
            cbor::from_slice(&self.0).map_err(|_| Error::MalformedTransaction)?;
        body.validate_basic()?;

        // Basic structure validation.
        if self.1.len() != body.auth_info.signer_info.len() {
            return Err(Error::MalformedTransaction);
        }

        // Verify all signatures.
        let ctx = signature::context::get_chain_context_for(SIGNATURE_CONTEXT_BASE);
        let mut public_keys = vec![];
        let mut signatures = vec![];
        for (si, auth_proof) in body.auth_info.signer_info.iter().zip(self.1.iter()) {
            let (mut batch_pks, mut batch_sigs) = si.address_spec.batch(auth_proof)?;
            public_keys.append(&mut batch_pks);
            signatures.append(&mut batch_sigs);
        }
        PublicKey::verify_batch_multisig(&ctx, &self.0, &public_keys, &signatures)
            .map_err(|_| Error::MalformedTransaction)?;

        Ok(body)
    }
}

/// Transaction.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transaction {
    #[serde(rename = "v")]
    pub version: u16,

    #[serde(rename = "call")]
    pub call: Call,

    #[serde(rename = "ai")]
    pub auth_info: AuthInfo,
}

impl Transaction {
    /// Perform basic validation on the transaction.
    pub fn validate_basic(&self) -> Result<(), Error> {
        if self.version != LATEST_TRANSACTION_VERSION {
            return Err(Error::UnsupportedVersion);
        }
        if self.auth_info.signer_info.is_empty() {
            return Err(Error::MalformedTransaction);
        }
        Ok(())
    }
}

/// Method call.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Call {
    #[serde(rename = "method")]
    pub method: String,

    #[serde(rename = "body")]
    pub body: cbor::Value,
}

/// Transaction authentication information.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthInfo {
    #[serde(rename = "si")]
    pub signer_info: Vec<SignerInfo>,

    #[serde(rename = "fee")]
    pub fee: Fee,
}

/// Transaction fee.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fee {
    #[serde(rename = "amount")]
    pub amount: token::BaseUnits,

    #[serde(rename = "gas")]
    pub gas: u64,
}

/// Common information that specifies an address as well as how to authenticate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AddressSpec {
    #[serde(rename = "signature")]
    Signature(PublicKey),
    #[serde(rename = "multisig")]
    Multisig(multisig::Config),
}

impl AddressSpec {
    pub fn address(&self) -> Address {
        match self {
            AddressSpec::Signature(public_key) => Address::from_pk(public_key),
            AddressSpec::Multisig(config) => Address::from_multisig(config),
        }
    }

    pub fn batch(&self, auth_proof: &AuthProof) -> Result<(Vec<PublicKey>, Vec<Signature>), Error> {
        Ok(match (self, auth_proof) {
            (AddressSpec::Signature(public_key), AuthProof::Signature(signature)) => {
                (vec![public_key.clone()], vec![signature.clone()])
            }
            (AddressSpec::Multisig(config), AuthProof::Multisig(signature_set)) => config
                .batch(signature_set)
                .map_err(|_| Error::MalformedTransaction)?,
            _ => return Err(Error::MalformedTransaction),
        })
    }
}

/// Transaction signer information.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignerInfo {
    #[serde(rename = "address_spec")]
    pub address_spec: AddressSpec,

    #[serde(rename = "nonce")]
    pub nonce: u64,
}

impl SignerInfo {
    /// Create a new signer info from public key and nonce.
    pub fn new(public_key: PublicKey, nonce: u64) -> Self {
        Self {
            address_spec: AddressSpec::Signature(public_key),
            nonce,
        }
    }
}

/// Call result.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum CallResult {
    #[serde(rename = "ok")]
    Ok(cbor::Value),

    #[serde(rename = "fail")]
    Failed {
        #[serde(rename = "module")]
        module: String,

        #[serde(rename = "code")]
        code: u32,

        #[serde(rename = "message")]
        #[serde(default)]
        #[serde(skip_serializing_if = "String::is_empty")]
        message: String,
    },
}

impl CallResult {
    /// Check whether the call result indicates a successful operation or not.
    pub fn is_success(&self) -> bool {
        match self {
            CallResult::Ok(_) => true,
            CallResult::Failed { .. } => false,
        }
    }
}
