//! A collection of common types used by the Oasis Contract SDK.

pub mod address;
pub mod crypto;
pub mod env;
pub mod event;
pub mod message;
pub mod modules;
pub mod storage;
pub mod token;

pub mod testing;

/// Unique stored code identifier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, cbor::Decode, cbor::Encode)]
#[cbor(transparent)]
pub struct CodeId(u64);

impl CodeId {
    /// Convert identifier to u64.
    pub fn as_u64(&self) -> u64 {
        self.0
    }

    /// Return the next identifier in sequence.
    pub fn increment(&self) -> Self {
        CodeId(self.0 + 1)
    }

    /// Convert identifier to storage key representation.
    pub fn to_storage_key(self) -> [u8; 8] {
        self.0.to_be_bytes()
    }
}

impl From<u64> for CodeId {
    fn from(v: u64) -> Self {
        CodeId(v)
    }
}

/// Unique deployed code instance identifier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, cbor::Decode, cbor::Encode)]
#[cbor(transparent)]
pub struct InstanceId(u64);

impl InstanceId {
    /// Convert identifier to u64.
    pub fn as_u64(&self) -> u64 {
        self.0
    }

    /// Return the next identifier in sequence.
    pub fn increment(&self) -> Self {
        InstanceId(self.0 + 1)
    }

    /// Convert identifier to storage key representation.
    pub fn to_storage_key(self) -> [u8; 8] {
        self.0.to_be_bytes()
    }
}

impl From<u64> for InstanceId {
    fn from(v: u64) -> Self {
        InstanceId(v)
    }
}

/// Format used for encoding the call (and output) information.
#[derive(Clone, Copy, Debug, PartialEq, Eq, cbor::Encode, cbor::Decode)]
#[repr(u8)]
#[cbor(with_default)]
pub enum CallFormat {
    /// Plain text call data.
    Plain = 0,
    /// Encrypted call data using X25519 for key exchange and Deoxys-II for symmetric encryption.
    EncryptedX25519DeoxysII = 1,
}

impl CallFormat {
    fn is_plain(&self) -> bool {
        matches!(self, CallFormat::Plain)
    }
}

impl Default for CallFormat {
    fn default() -> Self {
        Self::Plain
    }
}

#[cfg(feature = "oasis-runtime-sdk")]
impl From<oasis_runtime_sdk::types::transaction::CallFormat> for CallFormat {
    fn from(a: oasis_runtime_sdk::types::transaction::CallFormat) -> Self {
        use oasis_runtime_sdk::types::transaction::CallFormat as RuntimeCallFormat;

        match a {
            RuntimeCallFormat::Plain => Self::Plain,
            RuntimeCallFormat::EncryptedX25519DeoxysII => Self::EncryptedX25519DeoxysII,
        }
    }
}

/// Execution context.
///
/// Contains information that is useful on most invocations as it is always
/// included without requiring any explicit queries.
#[derive(Debug, Default, cbor::Decode, cbor::Encode)]
pub struct ExecutionContext {
    /// Contract instance identifier.
    pub instance_id: InstanceId,
    /// Contract instance address.
    pub instance_address: address::Address,
    /// Caller address.
    pub caller_address: address::Address,
    /// Tokens deposited by the caller.
    #[cbor(optional)]
    pub deposited_tokens: Vec<token::BaseUnits>,
    /// Read-only flag.
    ///
    /// A read-only call cannot make any changes to runtime state. Any attempt at modifying state
    /// will result in the call failing.
    #[cbor(optional)]
    pub read_only: bool,
    /// Transaction's call format.
    #[cbor(optional, skip_serializing_if = "CallFormat::is_plain")]
    pub call_format: CallFormat,
}

/// Contract execution result.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub enum ExecutionResult {
    #[cbor(rename = "ok")]
    Ok(ExecutionOk),

    #[cbor(rename = "fail")]
    Failed {
        #[cbor(optional)]
        module: String,

        code: u32,

        #[cbor(optional)]
        message: String,
    },
}

/// Result of a successful contract execution.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct ExecutionOk {
    /// Raw data returned from the contract.
    pub data: Vec<u8>,
    /// Messages emitted from the contract.
    #[cbor(optional)]
    pub messages: Vec<message::Message>,
    /// Events emitted from the contract.
    #[cbor(optional)]
    pub events: Vec<event::Event>,
}
