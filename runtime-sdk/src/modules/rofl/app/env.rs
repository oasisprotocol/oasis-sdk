use std::sync::Arc;

use anyhow::{anyhow, Result};
use tokio::sync::mpsc;

use crate::crypto::signature::Signer;

use super::{client, processor, App};

/// Application environment.
pub struct Environment<A: App> {
    client: client::Client<A>,
    signer: Arc<dyn Signer>,
    cmdq: mpsc::WeakSender<processor::Command>,
}

impl<A> Environment<A>
where
    A: App,
{
    /// Create a new environment talking to the given processor.
    pub(super) fn new(
        state: Arc<processor::State<A>>,
        cmdq: mpsc::WeakSender<processor::Command>,
    ) -> Self {
        Self {
            signer: state.signer.clone(),
            client: client::Client::new(state, cmdq.clone()),
            cmdq,
        }
    }

    /// Runtime client.
    pub fn client(&self) -> &client::Client<A> {
        &self.client
    }

    /// Transaction signer.
    pub fn signer(&self) -> &dyn Signer {
        self.signer.as_ref()
    }

    /// Send a command to the processor.
    pub(super) async fn send_command(&self, cmd: processor::Command) -> Result<()> {
        let cmdq = self
            .cmdq
            .upgrade()
            .ok_or(anyhow!("processor has shut down"))?;
        cmdq.send(cmd).await?;
        Ok(())
    }
}

impl<A> Clone for Environment<A>
where
    A: App,
{
    fn clone(&self) -> Self {
        Self {
            signer: self.signer.clone(),
            client: self.client.clone(),
            cmdq: self.cmdq.clone(),
        }
    }
}
