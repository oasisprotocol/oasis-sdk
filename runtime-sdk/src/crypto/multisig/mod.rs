use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::crypto::signature::{PublicKey, Signature};

/// Error.
#[derive(Error, Debug)]
pub enum Error {
    #[error("insufficient weight")]
    InsufficientWeight,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Signer {
    pub public_key: PublicKey,
    pub weight: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub signers: Vec<Signer>,
    pub threshold: u64,
}

impl Config {
    pub fn batch(
        &self,
        signature_set: &SignatureSet,
    ) -> Result<(Vec<PublicKey>, Vec<Signature>), Error> {
        let mut total = 0;
        let mut public_keys = vec![];
        let mut signatures = vec![];
        for (signer, signature_o) in self.signers.iter().zip(signature_set.signatures.iter()) {
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignatureSet {
    pub signatures: Vec<Option<Signature>>,
}
