use std::{collections::HashSet, sync::Arc};

use anyhow::{anyhow, Result};
use tokio::sync::{mpsc, oneshot};

use crate::{
    core::{
        consensus::{
            registry::{SGXConstraints, TEEHardware},
            state::{
                beacon::ImmutableState as BeaconState, registry::ImmutableState as RegistryState,
            },
        },
        enclave_rpc::{client::RpcClient, session},
        host::{self, Host as _},
    },
    crypto::signature::{PublicKey, Signer},
    enclave_rpc::{QueryRequest, METHOD_QUERY},
    modules::{
        accounts::API as _,
        core::{types::EstimateGasQuery, API as _},
    },
    state::CurrentState,
    storage::HostStore,
    types::{
        address::{Address, SignatureAddressSpec},
        token,
        transaction::{self, CallerAddress},
    },
    Runtime,
};

use super::{processor, App};

/// EnclaveRPC endpoint for communicating with the RONL component.
const ENCLAVE_RPC_ENDPOINT_RONL: &str = "ronl";

/// A runtime client meant for use within runtimes.
pub struct Client<A: App> {
    state: Arc<processor::State<A>>,
    cmdq: mpsc::WeakSender<processor::Command>,
}

impl<A> Client<A>
where
    A: App,
{
    /// Create a new runtime client.
    pub(super) fn new(
        state: Arc<processor::State<A>>,
        cmdq: mpsc::WeakSender<processor::Command>,
    ) -> Self {
        Self { state, cmdq }
    }

    /// Retrieve the latest known runtime round.
    pub async fn latest_round(&self) -> Result<u64> {
        let cmdq = self
            .cmdq
            .upgrade()
            .ok_or(anyhow!("processor has shut down"))?;
        let (tx, rx) = oneshot::channel();
        cmdq.send(processor::Command::GetLatestRound(tx)).await?;
        Ok(rx.await?)
    }

    /// Retrieve the nonce for the given account.
    pub async fn account_nonce(&self, round: u64, address: Address) -> Result<u64> {
        self.with_store_for_round(round, move || {
            Ok(<A::AttachTo as Runtime>::Accounts::get_nonce(address)?)
        })
        .await
    }

    /// Retrieve the gas price in the given denomination.
    pub async fn gas_price(&self, round: u64, denom: token::Denomination) -> Result<u128> {
        self.with_store_for_round(round, move || {
            <A::AttachTo as Runtime>::Core::min_gas_price(&denom)
                .ok_or(anyhow!("denomination not supported"))
        })
        .await
    }

    /// Securely query the on-chain runtime component.
    pub async fn query<Rq, Rs>(&self, round: u64, method: &str, args: Rq) -> Result<Rs>
    where
        Rq: cbor::Encode,
        Rs: cbor::Decode + Send + 'static,
    {
        // TODO: Consider using PolicyVerifier when it has the needed methods (and is async).
        let state = self.state.consensus_verifier.latest_state().await?;
        let runtime_id = self.state.host.get_runtime_id();
        let enclaves = tokio::task::spawn_blocking(move || -> Result<_> {
            let beacon = BeaconState::new(&state);
            let epoch = beacon.epoch()?;
            let registry = RegistryState::new(&state);
            let runtime = registry
                .runtime(&runtime_id)?
                .ok_or(anyhow!("runtime not available"))?;
            let ad = runtime
                .active_deployment(epoch)
                .ok_or(anyhow!("active runtime deployment not available"))?;

            match runtime.tee_hardware {
                TEEHardware::TEEHardwareIntelSGX => Ok(HashSet::from_iter(
                    ad.try_decode_tee::<SGXConstraints>()?.enclaves().clone(),
                )),
                _ => Err(anyhow!("unsupported TEE platform")),
            }
        })
        .await??;

        let identity = self
            .state
            .host
            .get_identity()
            .ok_or(anyhow!("local identity not available"))?
            .clone();
        let quote_policy = identity
            .quote_policy()
            .ok_or(anyhow!("quote policy not available"))?;
        let enclave_rpc = RpcClient::new_runtime(
            session::Builder::default()
                .use_endorsement(true)
                .quote_policy(Some(quote_policy))
                .local_identity(identity)
                .remote_enclaves(Some(enclaves)),
            self.state.host.clone(),
            ENCLAVE_RPC_ENDPOINT_RONL,
            vec![],
        );

        let response: Vec<u8> = enclave_rpc
            .secure_call(
                METHOD_QUERY,
                QueryRequest {
                    round,
                    method: method.to_string(),
                    args: cbor::to_vec(args),
                },
            )
            .await
            .into_result()?;

        Ok(cbor::from_slice(&response)?)
    }

    /// Securely perform gas estimation.
    pub async fn estimate_gas(&self, req: EstimateGasQuery) -> Result<u64> {
        let round = self.latest_round().await?;
        self.query(round, "core.EstimateGas", req).await
    }

    /// Sign a given transaction and submit it.
    ///
    /// This method supports multiple transaction signers.
    pub async fn multi_sign_and_submit_tx(
        &self,
        signers: &[&dyn Signer],
        mut tx: transaction::Transaction,
    ) -> Result<transaction::CallResult> {
        if signers.is_empty() {
            return Err(anyhow!("no signers specified"));
        }

        let round = self.latest_round().await?;

        // Resolve account nonces.
        for (idx, signer) in signers.iter().enumerate() {
            let sigspec = SignatureAddressSpec::try_from_pk(&signer.public_key())
                .ok_or(anyhow!("signature scheme not supported"))?;
            let address = Address::from_sigspec(&sigspec);
            let nonce = self.account_nonce(round, address).await?;

            tx.append_auth_signature(sigspec, nonce);

            // If gas is not set, perform estimation.
            if idx == 0 && tx.fee_gas() == 0 {
                let gas = self
                    .estimate_gas(EstimateGasQuery {
                        caller: if let PublicKey::Secp256k1(pk) = signer.public_key() {
                            Some(CallerAddress::EthAddress(
                                pk.to_eth_address().try_into().unwrap(),
                            ))
                        } else {
                            Some(CallerAddress::Address(address))
                        },
                        tx: tx.clone(),
                        propagate_failures: false,
                    })
                    .await?;
                tx.set_fee_gas(gas);
            }
        }

        // Determine gas price. Currently we always use the native denomination.
        let mgp = self.gas_price(round, token::Denomination::NATIVE).await?;
        let fee = mgp.saturating_mul(tx.fee_gas().into());
        tx.set_fee_amount(token::BaseUnits::new(fee, token::Denomination::NATIVE));

        // Sign the transaction.
        let mut tx = tx.prepare_for_signing();
        for signer in signers {
            tx.append_sign(*signer)?;
        }
        let tx = tx.finalize();

        // Submit the transaction.
        let result = self
            .state
            .host
            .submit_tx(
                cbor::to_vec(tx),
                host::SubmitTxOpts {
                    wait: true,
                    ..Default::default()
                },
            )
            .await?
            .ok_or(anyhow!("missing result"))?;
        cbor::from_slice(&result.output).map_err(|_| anyhow!("malformed result"))
    }

    /// Sign a given transaction and submit it.
    pub async fn sign_and_submit_tx(
        &self,
        signer: &dyn Signer,
        tx: transaction::Transaction,
    ) -> Result<transaction::CallResult> {
        self.multi_sign_and_submit_tx(&[signer], tx).await
    }

    /// Run a closure inside a `CurrentState` context with store for the given round.
    async fn with_store_for_round<F, R>(&self, round: u64, f: F) -> Result<R>
    where
        F: FnOnce() -> Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let store = self.store_for_round(round).await?;

        tokio::task::spawn_blocking(move || CurrentState::enter(store, f)).await?
    }

    /// Return a store corresponding to the given round.
    async fn store_for_round(&self, round: u64) -> Result<HostStore> {
        HostStore::new_for_round(
            self.state.host.clone(),
            &self.state.consensus_verifier,
            self.state.host.get_runtime_id(),
            round,
        )
        .await
    }
}

impl<A> Clone for Client<A>
where
    A: App,
{
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            cmdq: self.cmdq.clone(),
        }
    }
}
