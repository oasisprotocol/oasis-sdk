use oasis_runtime_sdk::{
    crypto::signature::Signature,
    types::transaction::{AuthProof, SignerInfo, TransactionRef},
};

pub trait Signer {
    fn sign(&self, tx: TransactionRef<'_>) -> Result<Vec<AuthProof>, Error>;

    fn info(&self) -> &[SignerInfo];
}

#[derive(Debug, thiserror::Error)]
pub enum Error {}
