use thiserror::Error;

use crate::crypto::signature::{PublicKey, Signature};

#[cfg(test)]
mod test;

/// Error.
#[derive(Error, Debug)]
pub enum Error {
    #[error("invalid config")]
    InvalidConfig,
    #[error("invalid signature set")]
    InvalidSignatureSet,
    #[error("insufficient weight")]
    InsufficientWeight,
}

/// One of the signers in a multisig configuration.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[cbor(no_default)]
pub struct Signer {
    /// The public key of the signer.
    pub public_key: PublicKey,
    /// The weight of the signer.
    pub weight: u64,
}

/// A set of signatures corresponding to a multisig configuration.
/// The indices match the configuration's `signers` vec.
pub type SignatureSet = [Option<Signature>];
/// A `SignatureSet` owned in a `Vec`.
pub type SignatureSetOwned = Vec<Option<Signature>>;

/// A multisig configuration.
/// A set of signers with total "weight" greater than or equal to a "threshold" can authenticate
/// for the configuration.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Config {
    /// The signers.
    pub signers: Vec<Signer>,
    /// The threshold.
    pub threshold: u64,
}

impl Config {
    /// Performs some sanity checks. This looks at the configuration only. There is no cryptographic
    /// verification of any signatures.
    pub fn validate_basic(&self) -> Result<(), Error> {
        if self.threshold == 0 {
            return Err(Error::InvalidConfig);
        }
        let mut total: u64 = 0;
        for (i, signer) in self.signers.iter().enumerate() {
            if self
                .signers
                .iter()
                .take(i)
                .any(|other_signer| signer.public_key == other_signer.public_key)
            {
                return Err(Error::InvalidConfig);
            }
            if signer.weight == 0 {
                return Err(Error::InvalidConfig);
            }
            total = total
                .checked_add(signer.weight)
                .ok_or(Error::InvalidConfig)?;
        }
        if total < self.threshold {
            return Err(Error::InvalidConfig);
        }
        Ok(())
    }

    /// Checks that the configuration and signature set are acceptable.
    /// Returns vectors of public keys and signatures for batch verification of included signatures.
    pub fn batch(
        &self,
        signature_set: &SignatureSet,
    ) -> Result<(Vec<PublicKey>, Vec<Signature>), Error> {
        self.validate_basic()?;
        if signature_set.len() != self.signers.len() {
            return Err(Error::InvalidSignatureSet);
        }
        let mut total = 0;
        let mut public_keys = vec![];
        let mut signatures = vec![];
        for (signer, signature_o) in self.signers.iter().zip(signature_set.iter()) {
            if let Some(signature) = signature_o {
                total += signer.weight;
                public_keys.push(signer.public_key.clone());
                signatures.push(signature.clone());
            }
        }
        if total < self.threshold {
            return Err(Error::InsufficientWeight);
        }
        Ok((public_keys, signatures))
    }
}
