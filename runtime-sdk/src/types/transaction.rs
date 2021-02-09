//! Transaction types.
use serde::{Deserialize, Serialize};
use thiserror::Error;

use oasis_core_runtime::common::cbor;

use crate::{
    crypto::signature::{PublicKey, Signature},
    types::token,
};

// TODO: Signature context: oasis-runtime-sdk/tx: v0 for chain H(<consensus-chain-context> || <runtime-id>)

/// The latest transaction format version.
pub const LATEST_TRANSACTION_VERSION: u16 = 1;

/// Error.
#[derive(Error, Debug)]
pub enum Error {
    #[error("unsupported version")]
    UnsupportedVersion,
    #[error("malformed transaction")]
    MalformedTransaction,
}

/// An unverified signed transaction.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnverifiedTransaction(#[serde(with = "serde_bytes")] Vec<u8>, Vec<Signature>);

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
        // XXX: Context.
        let signers: Vec<PublicKey> = body
            .auth_info
            .signer_info
            .iter()
            .map(|si| si.public_key.clone())
            .collect();
        PublicKey::verify_batch_multisig(b"TODO CTX", &self.0, &signers, &self.1)
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

/// Transaction signer information.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignerInfo {
    #[serde(rename = "pub")]
    pub public_key: PublicKey,

    #[serde(rename = "nonce")]
    pub nonce: u64,
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
