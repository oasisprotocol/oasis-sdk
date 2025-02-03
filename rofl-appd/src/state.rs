use oasis_runtime_sdk::{
    crypto::signature::Signer,
    modules::rofl::app::{client::SubmitTxOpts, prelude::*},
    types::transaction,
};

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
}
