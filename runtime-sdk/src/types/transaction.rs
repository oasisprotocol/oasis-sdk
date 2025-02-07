//! Transaction types.
use anyhow::anyhow;
use thiserror::Error;

use crate::{
    crypto::{
        multisig,
        signature::{self, PublicKey, Signature, Signer},
    },
    types::{
        address::{Address, SignatureAddressSpec},
        token,
    },
};

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
    #[error("signer not found in transaction")]
    SignerNotFound,
    #[error("failed to sign: {0}")]
    FailedToSign(#[from] signature::Error),
}

/// A container for data that authenticates a transaction.
#[derive(Clone, Default, Debug, cbor::Encode, cbor::Decode)]
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

    /// A non-serializable placeholder value.
    #[cbor(skip)]
    #[default]
    Invalid,
}

/// An unverified signed transaction.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[cbor(no_default)]
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

/// Transaction signer.
pub struct TransactionSigner {
    auth_info: AuthInfo,
    ut: UnverifiedTransaction,
}

impl TransactionSigner {
    /// Construct a new transaction signer for the given transaction.
    pub fn new(tx: Transaction) -> Self {
        let mut ts = Self {
            auth_info: tx.auth_info.clone(),
            ut: UnverifiedTransaction(cbor::to_vec(tx), vec![]),
        };
        ts.allocate_proofs();

        ts
    }

    /// Allocate proof structures based on the specified authentication info in the transaction.
    fn allocate_proofs(&mut self) {
        if !self.ut.1.is_empty() {
            return;
        }

        // Allocate proof slots.
        self.ut
            .1
            .resize_with(self.auth_info.signer_info.len(), Default::default);

        for (si, ap) in self.auth_info.signer_info.iter().zip(self.ut.1.iter_mut()) {
            match (&si.address_spec, ap) {
                (AddressSpec::Multisig(cfg), ap) => {
                    // Allocate multisig slots.
                    *ap = AuthProof::Multisig(vec![None; cfg.signers.len()]);
                }
                _ => continue,
            }
        }
    }

    /// Sign the transaction and append the signature.
    ///
    /// The signer must be specified in the `auth_info` field.
    pub fn append_sign<S>(&mut self, signer: &S) -> Result<(), Error>
    where
        S: Signer + ?Sized,
    {
        let ctx = signature::context::get_chain_context_for(SIGNATURE_CONTEXT_BASE);
        let signature = signer.sign(&ctx, &self.ut.0)?;

        let mut matched = false;
        for (si, ap) in self.auth_info.signer_info.iter().zip(self.ut.1.iter_mut()) {
            match (&si.address_spec, ap) {
                (AddressSpec::Signature(spec), ap) => {
                    if spec.public_key() != signer.public_key() {
                        continue;
                    }

                    matched = true;
                    *ap = AuthProof::Signature(signature.clone());
                }
                (AddressSpec::Multisig(cfg), AuthProof::Multisig(ref mut sigs)) => {
                    for (i, mss) in cfg.signers.iter().enumerate() {
                        if mss.public_key != signer.public_key() {
                            continue;
                        }

                        matched = true;
                        sigs[i] = Some(signature.clone());
                    }
                }
                _ => {
                    return Err(Error::MalformedTransaction(anyhow!(
                        "malformed address_spec"
                    )))
                }
            }
        }
        if !matched {
            return Err(Error::SignerNotFound);
        }
        Ok(())
    }

    /// Finalize the signing process and return the (signed) unverified transaction.
    pub fn finalize(self) -> UnverifiedTransaction {
        self.ut
    }
}

/// Transaction.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[cbor(no_default)]
pub struct Transaction {
    #[cbor(rename = "v")]
    pub version: u16,

    pub call: Call,

    #[cbor(rename = "ai")]
    pub auth_info: AuthInfo,
}

impl Transaction {
    /// Create a new (unsigned) transaction.
    pub fn new<B>(method: &str, body: B) -> Self
    where
        B: cbor::Encode,
    {
        Self {
            version: LATEST_TRANSACTION_VERSION,
            call: Call {
                format: CallFormat::Plain,
                method: method.to_string(),
                body: cbor::to_value(body),
                ..Default::default()
            },
            auth_info: Default::default(),
        }
    }

    /// Prepare this transaction for signing.
    pub fn prepare_for_signing(self) -> TransactionSigner {
        TransactionSigner::new(self)
    }

    /// Maximum amount of gas that the transaction can use.
    pub fn fee_gas(&self) -> u64 {
        self.auth_info.fee.gas
    }

    /// Set maximum amount of gas that the transaction can use.
    pub fn set_fee_gas(&mut self, gas: u64) {
        self.auth_info.fee.gas = gas;
    }

    /// Amount of fee to pay for transaction execution.
    pub fn fee_amount(&self) -> &token::BaseUnits {
        &self.auth_info.fee.amount
    }

    /// Set amount of fee to pay for transaction execution.
    pub fn set_fee_amount(&mut self, amount: token::BaseUnits) {
        self.auth_info.fee.amount = amount;
    }

    /// Set a proxy for paying the transaction fee.
    pub fn set_fee_proxy(&mut self, module: &str, id: &[u8]) {
        self.auth_info.fee.proxy = Some(FeeProxy {
            module: module.to_string(),
            id: id.to_vec(),
        });
    }

    /// Append a new transaction signer information to the transaction.
    pub fn append_signer_info(&mut self, address_spec: AddressSpec, nonce: u64) {
        self.auth_info.signer_info.push(SignerInfo {
            address_spec,
            nonce,
        })
    }

    /// Append a new transaction signer information with a signature address specification to the
    /// transaction.
    pub fn append_auth_signature(&mut self, spec: SignatureAddressSpec, nonce: u64) {
        self.append_signer_info(AddressSpec::Signature(spec), nonce);
    }

    /// Append a new transaction signer information with a multisig address specification to the
    /// transaction.
    pub fn append_auth_multisig(&mut self, cfg: multisig::Config, nonce: u64) {
        self.append_signer_info(AddressSpec::Multisig(cfg), nonce);
    }

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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, cbor::Encode, cbor::Decode)]
#[repr(u8)]
#[cbor(with_default)]
pub enum CallFormat {
    /// Plain text call data.
    #[default]
    Plain = 0,
    /// Encrypted call data using X25519 for key exchange and Deoxys-II for symmetric encryption.
    EncryptedX25519DeoxysII = 1,
}

impl CallFormat {
    /// Whether this call format is end-to-end encrypted.
    pub fn is_encrypted(&self) -> bool {
        match self {
            Self::Plain => false,
            Self::EncryptedX25519DeoxysII => true,
        }
    }
}

/// Method call.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct Call {
    /// Call format.
    #[cbor(optional)]
    pub format: CallFormat,
    /// Method name.
    #[cbor(optional)]
    pub method: String,
    /// Method body.
    pub body: cbor::Value,
    /// Read-only flag.
    ///
    /// A read-only call cannot make any changes to runtime state. Any attempt at modifying state
    /// will result in the call failing.
    #[cbor(optional, rename = "ro")]
    pub read_only: bool,
}

impl Default for Call {
    fn default() -> Self {
        Self {
            format: Default::default(),
            method: Default::default(),
            body: cbor::Value::Simple(cbor::SimpleValue::NullValue),
            read_only: false,
        }
    }
}

/// Transaction authentication information.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct AuthInfo {
    /// Transaction signer information.
    #[cbor(rename = "si")]
    pub signer_info: Vec<SignerInfo>,
    /// Fee payment information.
    pub fee: Fee,
    /// Earliest round when the transaction is valid.
    #[cbor(optional)]
    pub not_before: Option<u64>,
    /// Latest round when the transaction is valid.
    #[cbor(optional)]
    pub not_after: Option<u64>,
}

/// Transaction fee.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Fee {
    /// Amount of base units paid as fee for transaction processing.
    pub amount: token::BaseUnits,
    /// Maximum amount of gas paid for.
    #[cbor(optional)]
    pub gas: u64,
    /// Maximum amount of emitted consensus messages paid for. Zero means that up to the maximum
    /// number of per-batch messages can be emitted.
    #[cbor(optional)]
    pub consensus_messages: u32,
    /// Proxy which has authorized the fees to be paid.
    #[cbor(optional)]
    pub proxy: Option<FeeProxy>,
}

impl Fee {
    /// Calculates gas price from fee amount and gas.
    pub fn gas_price(&self) -> u128 {
        self.amount
            .amount()
            .checked_div(self.gas.into())
            .unwrap_or_default()
    }
}

/// Information about a fee proxy.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct FeeProxy {
    /// Module that will handle the proxy payment.
    pub module: String,
    /// Module-specific identifier that will handle fee payments for the transaction signer.
    pub id: Vec<u8>,
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
            CallerAddress::EthAddress(address) => Address::from_eth(address.as_ref()),
        }
    }

    /// Maps the caller address to one of the same type but with an all-zero address.
    pub fn zeroized(&self) -> Self {
        match self {
            CallerAddress::Address(_) => CallerAddress::Address(Default::default()),
            CallerAddress::EthAddress(_) => CallerAddress::EthAddress(Default::default()),
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
    /// Returns the public key when the address spec represents a single public key.
    pub fn public_key(&self) -> Option<PublicKey> {
        match self {
            AddressSpec::Signature(spec) => Some(spec.public_key()),
            _ => None,
        }
    }

    /// Derives the address.
    pub fn address(&self) -> Address {
        match self {
            AddressSpec::Signature(spec) => Address::from_sigspec(spec),
            AddressSpec::Multisig(config) => Address::from_multisig(config.clone()),
            AddressSpec::Internal(caller) => caller.address(),
        }
    }

    /// Derives the caller address.
    pub fn caller_address(&self) -> CallerAddress {
        match self {
            AddressSpec::Signature(SignatureAddressSpec::Secp256k1Eth(pk)) => {
                CallerAddress::EthAddress(pk.to_eth_address().try_into().unwrap())
            }
            AddressSpec::Internal(caller) => caller.clone(),
            _ => CallerAddress::Address(self.address()),
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
            (_, AuthProof::Invalid) => Err(Error::MalformedTransaction(anyhow!(
                "invalid auth proof in list"
            ))),
        }
    }
}

/// Transaction signer information.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[cbor(no_default)]
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
        message: String,
    },

    #[cbor(rename = "unknown")]
    Unknown(cbor::Value),
}

impl Default for CallResult {
    fn default() -> Self {
        Self::Unknown(cbor::Value::Simple(cbor::SimpleValue::NullValue))
    }
}

impl CallResult {
    /// Check whether the call result indicates a successful operation or not.
    pub fn is_success(&self) -> bool {
        !matches!(self, CallResult::Failed { .. })
    }

    /// Transforms `CallResult` into `anyhow::Result<cbor::Value>`, mapping `Ok(v)` and `Unknown(v)`
    /// to `Ok(v)` and `Failed` to `Err`.
    pub fn ok(self) -> anyhow::Result<cbor::Value> {
        match self {
            Self::Ok(v) | Self::Unknown(v) => Ok(v),
            Self::Failed {
                module,
                code,
                message,
            } => Err(anyhow!(
                "call failed: module={module} code={code}: {message}"
            )),
        }
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
            } => panic!("{module} reported failure with code {code}: {message}"),
        }
    }

    pub fn unwrap_failed(self) -> (String, u32) {
        match self {
            Self::Ok(_) | Self::Unknown(_) => panic!("call result indicates success"),
            Self::Failed { module, code, .. } => (module, code),
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
        let fee = Fee::default();
        assert_eq!(0, fee.gas_price(), "empty fee - gas price should be zero",);

        let fee = Fee {
            gas: 100,
            ..Default::default()
        };
        assert_eq!(
            0,
            fee.gas_price(),
            "empty fee amount - gas price should be zero",
        );

        let fee = Fee {
            amount: BaseUnits::new(1_000, Denomination::NATIVE),
            gas: 0,
            ..Default::default()
        };
        assert_eq!(0, fee.gas_price(), "empty fee 0 - gas price should be zero",);

        let fee = Fee {
            amount: BaseUnits::new(1_000, Denomination::NATIVE),
            gas: 10_000,
            ..Default::default()
        };
        assert_eq!(
            0,
            fee.gas_price(),
            "non empty fee - gas price should be zero"
        );

        let fee = Fee {
            amount: BaseUnits::new(1_000, Denomination::NATIVE),
            gas: 500,
            ..Default::default()
        };
        assert_eq!(2, fee.gas_price(), "non empty fee - gas price should match");
    }
}
