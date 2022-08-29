use thiserror::Error;

/// The latest incoming message format version.
pub const LATEST_INCOMING_MESSAGE_VERSION: u16 = 1;

/// Error.
#[derive(Debug, Error)]
pub enum Error {
    #[error("unsupported version")]
    UnsupportedVersion,
    #[error("malformed incoming message data: {0}")]
    MalformedTransaction(anyhow::Error),
}

/// Roothash incoming message data.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub struct IncomingMessageData {
    #[cbor(rename = "v")]
    pub version: u16,
    /// An embedded transaction (UnverifiedTransaction in runtimes using this SDK).
    /// The transaction doesn't need to be from the same account that sent the message.
    pub tx: Option<Vec<u8>>,
}

impl IncomingMessageData {
    pub fn noop() -> Self {
        Self {
            version: LATEST_INCOMING_MESSAGE_VERSION,
            tx: None,
        }
    }

    pub fn validate_basic(&self) -> Result<(), Error> {
        if self.version != LATEST_INCOMING_MESSAGE_VERSION {
            return Err(Error::UnsupportedVersion);
        }
        Ok(())
    }
}
