use std::{collections::BTreeMap, sync::Arc};

use anyhow::{anyhow, Result};
use tokio::sync::mpsc;

use crate::{
    core::{common::namespace::Namespace, host::Host, identity::Identity, protocol::Protocol},
    crypto::signature::Signer,
};

use super::{client, processor, App};

/// Application environment.
pub struct Environment<A: App> {
    app: Arc<A>,
    client: client::Client<A>,
    signer: Arc<dyn Signer>,
    identity: Arc<Identity>,
    host: Arc<Protocol>,
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
            app: state.app.clone(),
            signer: state.signer.clone(),
            identity: state.identity.clone(),
            host: state.host.clone(),
            client: client::Client::new(state, cmdq.clone()),
            cmdq,
        }
    }

    /// Application instance.
    pub fn app(&self) -> Arc<A> {
        self.app.clone()
    }

    /// Runtime client.
    pub fn client(&self) -> &client::Client<A> {
        &self.client
    }

    /// Transaction signer.
    pub fn signer(&self) -> Arc<dyn Signer> {
        self.signer.clone()
    }

    /// Runtime identity.
    pub fn identity(&self) -> Arc<Identity> {
        self.identity.clone()
    }

    /// Host interface.
    pub fn host(&self) -> Arc<dyn Host> {
        self.host.clone()
    }

    /// Runtime identifier.
    pub fn runtime_id(&self) -> Namespace {
        self.host.get_runtime_id()
    }

    /// Untrusted local runtime configuration defined by the node operator where the node hosting
    /// the ROFL app is running.
    pub fn untrusted_local_config(&self) -> BTreeMap<String, cbor::Value> {
        self.host.get_host_info().local_config
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
            app: self.app.clone(),
            signer: self.signer.clone(),
            identity: self.identity.clone(),
            host: self.host.clone(),
            client: self.client.clone(),
            cmdq: self.cmdq.clone(),
        }
    }
}
