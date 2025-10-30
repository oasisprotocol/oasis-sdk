use oasis_runtime_sdk::{crypto::signature::Signer, types::transaction};
use rofl_app_core::{client::SubmitTxOpts, prelude::*};

/// ROFL app environment.
#[async_trait]
pub trait Env: Send + Sync {
    /// ROFL app identifier of the running application.
    fn app_id(&self) -> AppId;

    /// Transaction signer.
    fn signer(&self) -> Arc<dyn Signer>;

    /// Sign a given transaction, submit it and wait for block inclusion.
    async fn sign_and_submit_tx(
        &self,
        signer: Arc<dyn Signer>,
        tx: transaction::Transaction,
        opts: SubmitTxOpts,
    ) -> Result<transaction::CallResult>;

    /// Securely query the on-chain paratime state.
    async fn query(&self, method: &str, args: Vec<u8>) -> Result<Vec<u8>>;
}

pub(crate) struct EnvImpl<A: App> {
    env: Environment<A>,
}

impl<A: App> EnvImpl<A> {
    pub fn new(env: Environment<A>) -> Self {
        Self { env }
    }
}

#[async_trait]
impl<A: App> Env for EnvImpl<A> {
    fn app_id(&self) -> AppId {
        A::id()
    }

    fn signer(&self) -> Arc<dyn Signer> {
        self.env.signer()
    }

    async fn sign_and_submit_tx(
        &self,
        signer: Arc<dyn Signer>,
        tx: transaction::Transaction,
        opts: SubmitTxOpts,
    ) -> Result<transaction::CallResult> {
        self.env
            .client()
            .multi_sign_and_submit_tx_opts(&[signer], tx, opts)
            .await
    }

    async fn query(&self, method: &str, args: Vec<u8>) -> Result<Vec<u8>> {
        let args: cbor::Value = cbor::from_slice(&args)?;
        let round = self.env.client().latest_round().await?;
        let result: cbor::Value = self.env.client().query(round, method, args).await?;
        Ok(cbor::to_vec(result))
    }
}
