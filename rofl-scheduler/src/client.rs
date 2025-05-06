//! ROFL market client.
use std::collections::BTreeMap;

use anyhow::Result;
use oasis_runtime_sdk::{modules::rofl::app::prelude::*, types::address::Address};
use oasis_runtime_sdk_rofl_market::{
    self as market,
    types::{
        Instance, InstanceAccept, InstanceId, InstanceQuery, InstanceRemove, InstanceUpdate, Offer,
        ProviderQuery, QueuedCommand, Update,
    },
};

use super::SchedulerApp;

/// ROFL market client.
pub struct MarketClient {
    env: Environment<SchedulerApp>,
    provider: Address,
}

impl MarketClient {
    /// Create a new ROFL market client.
    pub fn new(env: Environment<SchedulerApp>, provider: Address) -> Self {
        Self { env, provider }
    }

    /// Create a ROFL market query client for the latest round at the time of creation.
    pub async fn queries_at_latest(self: &Arc<Self>) -> Result<Arc<MarketQueryClient>> {
        let round = self.env.client().latest_round().await?;
        Ok(self.queries(round))
    }

    /// Create a ROFL market query client for the given round.
    pub fn queries(self: &Arc<Self>, round: u64) -> Arc<MarketQueryClient> {
        Arc::new(MarketQueryClient::new(self.clone(), round))
    }

    /// Issue a transaction to accept multiple instances.
    pub async fn accept_instances(
        &self,
        ids: Vec<InstanceId>,
        metadata: BTreeMap<String, String>,
    ) -> Result<()> {
        let tx = self.env.app().new_transaction(
            "roflmarket.InstanceAccept",
            InstanceAccept {
                provider: self.provider,
                ids,
                metadata,
            },
        );

        let _ = self
            .env
            .client()
            .sign_and_submit_tx(self.env.signer(), tx)
            .await?;

        Ok(())
    }

    /// Issue a transaction to remove the given instance.
    pub async fn remove_instance(&self, id: InstanceId) -> Result<()> {
        let tx = self.env.app().new_transaction(
            "roflmarket.InstanceRemove",
            InstanceRemove {
                provider: self.provider,
                id,
            },
        );

        let _ = self
            .env
            .client()
            .sign_and_submit_tx(self.env.signer(), tx)
            .await?;

        Ok(())
    }

    /// Issue a transaction to update the given instances.
    pub async fn update_instances(&self, updates: Vec<Update>) -> Result<()> {
        let tx = self.env.app().new_transaction(
            "roflmarket.InstanceUpdate",
            InstanceUpdate {
                provider: self.provider,
                updates,
            },
        );

        let _ = self
            .env
            .client()
            .sign_and_submit_tx(self.env.signer(), tx)
            .await?;

        Ok(())
    }

    /// Issue a transaction to claim payment for the given instances.
    pub async fn claim_payment(&self, instances: Vec<InstanceId>) -> Result<()> {
        let tx = self.env.app().new_transaction(
            "roflmarket.InstanceClaimPayment",
            market::types::InstanceClaimPayment {
                provider: self.provider,
                instances,
            },
        );

        let _ = self
            .env
            .client()
            .sign_and_submit_tx(self.env.signer(), tx)
            .await?;

        Ok(())
    }
}

/// ROFL market query client.
pub struct MarketQueryClient {
    parent: Arc<MarketClient>,
    round: u64,
}

impl MarketQueryClient {
    fn new(parent: Arc<MarketClient>, round: u64) -> Self {
        Self { parent, round }
    }

    /// Round used for the queries.
    pub fn round(&self) -> u64 {
        self.round
    }

    /// Query all provider's instances.
    pub async fn instances(&self) -> Result<Vec<Instance>> {
        self.parent
            .env
            .client()
            .query(
                self.round,
                "roflmarket.Instances",
                ProviderQuery {
                    provider: self.parent.provider,
                },
            )
            .await
    }

    /// Query all queued commands of a given instance.
    pub async fn instance_commands(&self, id: InstanceId) -> Result<Vec<QueuedCommand>> {
        self.parent
            .env
            .client()
            .query(
                self.round,
                "roflmarket.InstanceCommands",
                InstanceQuery {
                    provider: self.parent.provider,
                    id,
                },
            )
            .await
    }

    /// Query all provider's offers.
    pub async fn offers(&self) -> Result<Vec<Offer>> {
        self.parent
            .env
            .client()
            .query(
                self.round,
                "roflmarket.Offers",
                ProviderQuery {
                    provider: self.parent.provider,
                },
            )
            .await
    }
}
