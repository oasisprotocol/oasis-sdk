//! Transaction types.
use anyhow::anyhow;
use thiserror::Error;

use crate::{
    crypto::{
        multisig,
        signature::{self, PublicKey, Signature},
    },
    types::{
        address,
        address::{Address, SignatureAddressSpec},
        token,
    },
};

// Re-export TransactionWeight type.
pub use oasis_core_runtime::types::TransactionWeight;

/// Transaction signature domain separation context base.
pub const SIGNATURE_CONTEXT_BASE: &[u8] = b"oasis-runtime-sdk/tx: v0";
/// The latest transaction format version.
pub const LATEST_TRANSACTION_VERSION: u16 = 1;

/// Error.
#[derive(Debug, Error)]
pub enum Error {
    #[error("unsupported version")]
    UnsupportedVersion,
    #[error("malformed transaction: {0}")]
    MalformedTransaction(anyhow::Error),
}

/// A container for data that authenticates a transaction.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub enum AuthProof {
    /// For _signature_ authentication.
    #[cbor(rename = "signature")]
    Signature(Signature),
    /// For _multisig_ authentication.
    #[cbor(rename = "multisig")]
    Multisig(multisig::SignatureSetOwned),
    /// A flag to use module-controlled decoding. The string is an encoding scheme name that a
    /// module must handle. The scheme name must not be empty.
    #[cbor(rename = "module")]
    Module(String),
}

/// An unverified signed transaction.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct UnverifiedTransaction(pub Vec<u8>, pub Vec<AuthProof>);

impl UnverifiedTransaction {
    /// Verify and deserialize the unverified transaction.
    pub fn verify(self) -> Result<Transaction, Error> {
        // Deserialize the inner body.
        let body: Transaction =
            cbor::from_slice(&self.0).map_err(|e| Error::MalformedTransaction(e.into()))?;
        body.validate_basic()?;

        // Basic structure validation.
        if self.1.len() != body.auth_info.signer_info.len() {
            return Err(Error::MalformedTransaction(anyhow!(
                "unexpected number of auth proofs. expected {} but found {}",
                body.auth_info.signer_info.len(),
                self.1.len()
            )));
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
            .map_err(|e| Error::MalformedTransaction(e.into()))?;

        Ok(body)
    }
}

/// Transaction.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct Transaction {
    #[cbor(rename = "v")]
    pub version: u16,

    pub call: Call,

    #[cbor(rename = "ai")]
    pub auth_info: AuthInfo,
}

impl Transaction {
    /// Perform basic validation on the transaction.
    pub fn validate_basic(&self) -> Result<(), Error> {
        if self.version != LATEST_TRANSACTION_VERSION {
            return Err(Error::UnsupportedVersion);
        }
        if self.auth_info.signer_info.is_empty() {
            return Err(Error::MalformedTransaction(anyhow!(
                "transaction has no signers"
            )));
        }
        Ok(())
    }
}

/// Format used for encoding the call (and output) information.
#[derive(Clone, Copy, Debug, PartialEq, Eq, cbor::Encode, cbor::Decode)]
#[repr(u8)]
pub enum CallFormat {
    /// Plain text call data.
    Plain = 0,
    /// Encrypted call data using X25519 for key exchange and Deoxys-II for symmetric encryption.
    EncryptedX25519DeoxysII = 1,
}

impl Default for CallFormat {
    fn default() -> Self {
        Self::Plain
    }
}

/// Method call.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct Call {
    /// Call format.
    #[cbor(optional, default)]
    pub format: CallFormat,
    /// Method name.
    #[cbor(optional, default, skip_serializing_if = "String::is_empty")]
    pub method: String,
    /// Method body.
    pub body: cbor::Value,
}

/// Transaction authentication information.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct AuthInfo {
    #[cbor(rename = "si")]
    pub signer_info: Vec<SignerInfo>,
    pub fee: Fee,
}

/// Transaction fee.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Fee {
    /// Amount of base units paid as fee for transaction processing.
    pub amount: token::BaseUnits,
    /// Maximum amount of gas paid for.
    #[cbor(optional, default, skip_serializing_if = "num_traits::Zero::is_zero")]
    pub gas: u64,
    /// Maximum amount of emitted consensus messages paid for.
    #[cbor(optional, default, skip_serializing_if = "num_traits::Zero::is_zero")]
    pub consensus_messages: u32,
}

impl Fee {
    /// Caculates gas price from fee amount and gas.
    pub fn gas_price(&self) -> u128 {
        self.amount
            .amount()
            .checked_div(self.gas.into())
            .unwrap_or_default()
    }
}

/// A caller address.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub enum CallerAddress {
    #[cbor(rename = "address")]
    Address(Address),
    #[cbor(rename = "eth_address")]
    EthAddress([u8; 20]),
}

impl CallerAddress {
    /// Derives the address.
    pub fn address(&self) -> Address {
        match self {
            CallerAddress::Address(address) => *address,
            CallerAddress::EthAddress(address) => Address::new(
                address::ADDRESS_V0_SECP256K1ETH_CONTEXT,
                address::ADDRESS_V0_VERSION,
                address.as_ref(),
            ),
        }
    }
}

/// Common information that specifies an address as well as how to authenticate.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub enum AddressSpec {
    /// For _signature_ authentication.
    #[cbor(rename = "signature")]
    Signature(SignatureAddressSpec),
    /// For _multisig_ authentication.
    #[cbor(rename = "multisig")]
    Multisig(multisig::Config),

    /// For internal child calls (cannot be serialized/deserialized).
    #[cbor(skip)]
    Internal(CallerAddress),
}

impl AddressSpec {
    /// Derives the address.
    pub fn address(&self) -> Address {
        match self {
            AddressSpec::Signature(spec) => Address::from_sigspec(spec),
            AddressSpec::Multisig(config) => Address::from_multisig(config.clone()),
            AddressSpec::Internal(caller) => caller.address(),
        }
    }

    /// Checks that the address specification and the authentication proof are acceptable.
    /// Returns vectors of public keys and signatures for batch verification of included signatures.
    pub fn batch(&self, auth_proof: &AuthProof) -> Result<(Vec<PublicKey>, Vec<Signature>), Error> {
        match (self, auth_proof) {
            (AddressSpec::Signature(spec), AuthProof::Signature(signature)) => {
                Ok((vec![spec.public_key()], vec![signature.clone()]))
            }
            (AddressSpec::Multisig(config), AuthProof::Multisig(signature_set)) => Ok(config
                .batch(signature_set)
                .map_err(|e| Error::MalformedTransaction(e.into()))?),
            (AddressSpec::Signature(_), AuthProof::Multisig(_)) => {
                Err(Error::MalformedTransaction(anyhow!(
                    "transaction signer used a single signature, but auth proof was multisig"
                )))
            }
            (AddressSpec::Multisig(_), AuthProof::Signature(_)) => {
                Err(Error::MalformedTransaction(anyhow!(
                    "transaction signer used multisig, but auth proof was a single signature"
                )))
            }
            (AddressSpec::Internal(_), _) => Err(Error::MalformedTransaction(anyhow!(
                "transaction signer used internal address spec"
            ))),
            (_, AuthProof::Module(_)) => Err(Error::MalformedTransaction(anyhow!(
                "module-controlled decoding flag in auth proof list"
            ))),
        }
    }
}

/// Transaction signer information.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct SignerInfo {
    pub address_spec: AddressSpec,
    pub nonce: u64,
}

impl SignerInfo {
    /// Create a new signer info from a signature address specification and nonce.
    pub fn new_sigspec(spec: SignatureAddressSpec, nonce: u64) -> Self {
        Self {
            address_spec: AddressSpec::Signature(spec),
            nonce,
        }
    }

    /// Create a new signer info from a multisig configuration and a nonce.
    pub fn new_multisig(config: multisig::Config, nonce: u64) -> Self {
        Self {
            address_spec: AddressSpec::Multisig(config),
            nonce,
        }
    }
}

/// Call result.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub enum CallResult {
    #[cbor(rename = "ok")]
    Ok(cbor::Value),

    #[cbor(rename = "fail")]
    Failed {
        module: String,
        code: u32,

        #[cbor(optional)]
        #[cbor(default)]
        #[cbor(skip_serializing_if = "String::is_empty")]
        message: String,
    },

    #[cbor(rename = "unknown")]
    Unknown(cbor::Value),
}

impl CallResult {
    /// Check whether the call result indicates a successful operation or not.
    pub fn is_success(&self) -> bool {
        !matches!(self, CallResult::Failed { .. })
    }
}

#[cfg(any(test, feature = "test"))]
impl CallResult {
    pub fn unwrap(self) -> cbor::Value {
        match self {
            Self::Ok(v) | Self::Unknown(v) => v,
            Self::Failed {
                module,
                code,
                message,
            } => panic!(
                "{} reported failure with code {}: {}",
                module, code, message
            ),
        }
    }

    pub fn into_call_result(self) -> Option<crate::module::CallResult> {
        Some(match self {
            Self::Ok(v) => crate::module::CallResult::Ok(v),
            Self::Failed {
                module,
                code,
                message,
            } => crate::module::CallResult::Failed {
                module,
                code,
                message,
            },
            Self::Unknown(_) => return None,
        })
    }
}

#[cfg(test)]
mod test {
    use crate::types::token::{BaseUnits, Denomination};

    use super::*;

    #[test]
    fn test_fee_gas_price() {
        let fee = Fee {
            amount: Default::default(),
            gas: 0,
            consensus_messages: 0,
        };
        assert_eq!(0, fee.gas_price(), "empty fee - gas price should be zero",);

        let fee = Fee {
            amount: Default::default(),
            gas: 100,
            consensus_messages: 0,
        };
        assert_eq!(
            0,
            fee.gas_price(),
            "empty fee amount - gas price should be zero",
        );

        let fee = Fee {
            amount: BaseUnits::new(1_000, Denomination::NATIVE),
            gas: 0,
            consensus_messages: 0,
        };
        assert_eq!(0, fee.gas_price(), "empty fee 0 - gas price should be zero",);

        let fee = Fee {
            amount: BaseUnits::new(1_000, Denomination::NATIVE),
            gas: 10_000,
            consensus_messages: 0,
        };
        assert_eq!(
            0,
            fee.gas_price(),
            "non empty fee - gas price should be zero"
        );

        let fee = Fee {
            amount: BaseUnits::new(1_000, Denomination::NATIVE),
            gas: 500,
            consensus_messages: 0,
        };
        assert_eq!(2, fee.gas_price(), "non empty fee - gas price should match");
    }
}
