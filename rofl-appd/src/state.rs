use oasis_runtime_sdk::{crypto::signature::Signer, types::transaction};
use rofl_app_core::{client::SubmitTxOpts, prelude::*};
use oasis_runtime_sdk::types::transaction::UnverifiedTransaction;


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

    /// Submit a prepared UnverifiedTransaction directly.
    async fn submit_prepared_tx(
        &self,
        prepared_tx: UnverifiedTransaction,
    ) -> Result<transaction::CallResult>;


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

    async fn submit_prepared_tx(
        &self,
        prepared_tx: UnverifiedTransaction,
    ) -> Result<transaction::CallResult> {
        self.env
            .client()
            .submit_prepared_tx(prepared_tx)
            .await
    }


}
