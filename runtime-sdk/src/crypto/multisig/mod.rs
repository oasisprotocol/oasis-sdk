use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::crypto::signature::{PublicKey, Signature};

#[cfg(test)]
mod test;

/// Error.
#[derive(Error, Debug)]
pub enum Error {
    #[error("invalid config")]
    InvalidConfig,
    #[error("insufficient weight")]
    InsufficientWeight,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Signer {
    pub public_key: PublicKey,
    pub weight: u64,
}

pub type SignatureSet = [Option<Signature>];
pub type SignatureSetOwned = Vec<Option<Signature>>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub signers: Vec<Signer>,
    pub threshold: u64,
}

impl Config {
    pub fn verify(&self) -> Result<(), Error> {
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

    pub fn batch(
        &self,
        signature_set: &SignatureSet,
    ) -> Result<(Vec<PublicKey>, Vec<Signature>), Error> {
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
