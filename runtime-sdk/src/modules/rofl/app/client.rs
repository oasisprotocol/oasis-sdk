use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
    time::Duration,
};

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
    modules::{accounts::types::NonceQuery, core::types::EstimateGasQuery},
    state::CurrentState,
    storage::HostStore,
    types::{
        address::{Address, SignatureAddressSpec},
        token,
        transaction::{self, CallerAddress},
    },
};

use super::{processor, App};

/// Size of various command queues.
const CMDQ_BACKLOG: usize = 16;

/// EnclaveRPC endpoint for communicating with the RONL component.
const ENCLAVE_RPC_ENDPOINT_RONL: &str = "ronl";

/// A runtime client meant for use within runtimes.
pub struct Client<A: App> {
    imp: ClientImpl<A>,
    submission_mgr: Arc<SubmissionManager<A>>,
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
        let imp = ClientImpl::new(state, cmdq);
        let mut submission_mgr = SubmissionManager::new(imp.clone());
        submission_mgr.start();

        Self {
            imp,
            submission_mgr: Arc::new(submission_mgr),
        }
    }

    /// Retrieve the latest known runtime round.
    pub async fn latest_round(&self) -> Result<u64> {
        self.imp.latest_round().await
    }

    /// Retrieve the nonce for the given account.
    pub async fn account_nonce(&self, round: u64, address: Address) -> Result<u64> {
        self.imp.account_nonce(round, address).await
    }

    /// Retrieve the gas price in the given denomination.
    pub async fn gas_price(&self, round: u64, denom: &token::Denomination) -> Result<u128> {
        self.imp.gas_price(round, denom).await
    }

    /// Securely query the on-chain runtime component.
    pub async fn query<Rq, Rs>(&self, round: u64, method: &str, args: Rq) -> Result<Rs>
    where
        Rq: cbor::Encode,
        Rs: cbor::Decode + Send + 'static,
    {
        self.imp.query(round, method, args).await
    }

    /// Securely perform gas estimation.
    pub async fn estimate_gas(&self, req: EstimateGasQuery) -> Result<u64> {
        self.imp.estimate_gas(req).await
    }

    /// Sign a given transaction, submit it and wait for block inclusion.
    ///
    /// This method supports multiple transaction signers.
    pub async fn multi_sign_and_submit_tx(
        &self,
        signers: &[Arc<dyn Signer>],
        tx: transaction::Transaction,
    ) -> Result<transaction::CallResult> {
        self.submission_mgr
            .multi_sign_and_submit_tx(signers, tx)
            .await
    }

    /// Sign a given transaction, submit it and wait for block inclusion.
    pub async fn sign_and_submit_tx(
        &self,
        signer: Arc<dyn Signer>,
        tx: transaction::Transaction,
    ) -> Result<transaction::CallResult> {
        self.multi_sign_and_submit_tx(&[signer], tx).await
    }

    /// Run a closure inside a `CurrentState` context with store for the given round.
    pub async fn with_store_for_round<F, R>(&self, round: u64, f: F) -> Result<R>
    where
        F: FnOnce() -> Result<R> + Send + 'static,
        R: Send + 'static,
    {
        self.imp.with_store_for_round(round, f).await
    }

    /// Return a store corresponding to the given round.
    pub async fn store_for_round(&self, round: u64) -> Result<HostStore> {
        self.imp.store_for_round(round).await
    }
}

impl<A> Clone for Client<A>
where
    A: App,
{
    fn clone(&self) -> Self {
        Self {
            imp: self.imp.clone(),
            submission_mgr: self.submission_mgr.clone(),
        }
    }
}

struct ClientImpl<A: App> {
    state: Arc<processor::State<A>>,
    cmdq: mpsc::WeakSender<processor::Command>,
}

impl<A> ClientImpl<A>
where
    A: App,
{
    fn new(state: Arc<processor::State<A>>, cmdq: mpsc::WeakSender<processor::Command>) -> Self {
        Self { state, cmdq }
    }

    /// Retrieve the latest known runtime round.
    async fn latest_round(&self) -> Result<u64> {
        let cmdq = self
            .cmdq
            .upgrade()
            .ok_or(anyhow!("processor has shut down"))?;
        let (tx, rx) = oneshot::channel();
        cmdq.send(processor::Command::GetLatestRound(tx)).await?;
        Ok(rx.await?)
    }

    /// Retrieve the nonce for the given account.
    async fn account_nonce(&self, round: u64, address: Address) -> Result<u64> {
        self.query(round, "accounts.Nonce", NonceQuery { address })
            .await
    }

    /// Retrieve the gas price in the given denomination.
    async fn gas_price(&self, round: u64, denom: &token::Denomination) -> Result<u128> {
        let mgp: BTreeMap<token::Denomination, u128> =
            self.query(round, "core.MinGasPrice", ()).await?;
        mgp.get(denom)
            .ok_or(anyhow!("denomination not supported"))
            .copied()
    }

    /// Securely query the on-chain runtime component.
    async fn query<Rq, Rs>(&self, round: u64, method: &str, args: Rq) -> Result<Rs>
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
    async fn estimate_gas(&self, req: EstimateGasQuery) -> Result<u64> {
        let round = self.latest_round().await?;
        self.query(round, "core.EstimateGas", req).await
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

impl<A> Clone for ClientImpl<A>
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

enum Cmd {
    SubmitTx(
        Vec<Arc<dyn Signer>>,
        transaction::Transaction,
        oneshot::Sender<Result<transaction::CallResult>>,
    ),
}

/// Transaction submission manager for avoiding nonce conflicts.
struct SubmissionManager<A: App> {
    imp: Option<SubmissionManagerImpl<A>>,
    cmdq_tx: mpsc::Sender<Cmd>,
}

impl<A> SubmissionManager<A>
where
    A: App,
{
    /// Create a new submission manager.
    fn new(client: ClientImpl<A>) -> Self {
        let (tx, rx) = mpsc::channel(CMDQ_BACKLOG);

        Self {
            imp: Some(SubmissionManagerImpl {
                client,
                cmdq_rx: rx,
            }),
            cmdq_tx: tx,
        }
    }

    /// Start the submission manager task.
    fn start(&mut self) {
        if let Some(imp) = self.imp.take() {
            imp.start();
        }
    }

    /// Sign a given transaction, submit it and wait for block inclusion.
    async fn multi_sign_and_submit_tx(
        &self,
        signers: &[Arc<dyn Signer>],
        tx: transaction::Transaction,
    ) -> Result<transaction::CallResult> {
        let (ch, rx) = oneshot::channel();
        self.cmdq_tx
            .send(Cmd::SubmitTx(signers.to_vec(), tx, ch))
            .await?;
        rx.await?
    }
}

struct SubmissionManagerImpl<A: App> {
    client: ClientImpl<A>,
    cmdq_rx: mpsc::Receiver<Cmd>,
}

impl<A> SubmissionManagerImpl<A>
where
    A: App,
{
    /// Start the submission manager task.
    fn start(self) {
        tokio::task::spawn(self.run());
    }

    /// Run the submission manager task.
    async fn run(mut self) {
        let (notify_tx, mut notify_rx) = mpsc::channel::<HashSet<PublicKey>>(CMDQ_BACKLOG);
        let mut queue: Vec<Cmd> = Vec::new();
        let mut pending: HashSet<PublicKey> = HashSet::new();

        loop {
            tokio::select! {
                // Process incoming commands.
                Some(cmd) = self.cmdq_rx.recv() => queue.push(cmd),

                // Process incoming completion notifications.
                Some(signers) = notify_rx.recv() => {
                    for pk in signers {
                        pending.remove(&pk);
                    }
                },

                else => break,
            }

            // Check if there is anything in the queue that can be executed without conflicts.
            let mut new_queue = Vec::with_capacity(queue.len());
            for cmd in queue {
                match cmd {
                    Cmd::SubmitTx(signers, tx, ch) => {
                        // Check if transaction can be executed (no conflicts with in-flight txs).
                        let signer_set =
                            HashSet::from_iter(signers.iter().map(|signer| signer.public_key()));
                        if !signer_set.is_disjoint(&pending) {
                            // Defer any non-executable commands.
                            new_queue.push(Cmd::SubmitTx(signers, tx, ch));
                            continue;
                        }
                        // Include all signers in the pending set.
                        pending.extend(signer_set.iter().cloned());

                        // Execute in a separate task.
                        let client = self.client.clone();
                        let notify_tx = notify_tx.clone();

                        tokio::spawn(async move {
                            let result = Self::multi_sign_and_submit_tx(client, &signers, tx).await;
                            let _ = ch.send(result);

                            // Notify the submission manager task that submission is done.
                            let _ = notify_tx.send(signer_set).await;
                        });
                    }
                }
            }
            queue = new_queue;
        }
    }

    /// Sign a given transaction, submit it and wait for block inclusion.
    async fn multi_sign_and_submit_tx(
        client: ClientImpl<A>,
        signers: &[Arc<dyn Signer>],
        mut tx: transaction::Transaction,
    ) -> Result<transaction::CallResult> {
        if signers.is_empty() {
            return Err(anyhow!("no signers specified"));
        }

        // Resolve signer addresses.
        let addresses = signers
            .iter()
            .map(|signer| -> Result<_> {
                let sigspec = SignatureAddressSpec::try_from_pk(&signer.public_key())
                    .ok_or(anyhow!("signature scheme not supported"))?;
                Ok((Address::from_sigspec(&sigspec), sigspec))
            })
            .collect::<Result<Vec<_>>>()?;

        let round = client.latest_round().await?;

        // Resolve account nonces.
        for (address, sigspec) in &addresses {
            let nonce = client.account_nonce(round, *address).await?;

            tx.append_auth_signature(sigspec.clone(), nonce);
        }

        // Perform gas estimation after all signer infos have been added as otherwise we may
        // underestimate the amount of gas needed.
        if tx.fee_gas() == 0 {
            let signer = &signers[0]; // Checked to have at least one signer above.
            let gas = client
                .estimate_gas(EstimateGasQuery {
                    caller: if let PublicKey::Secp256k1(pk) = signer.public_key() {
                        Some(CallerAddress::EthAddress(
                            pk.to_eth_address().try_into().unwrap(),
                        ))
                    } else {
                        Some(CallerAddress::Address(addresses[0].0)) // Checked above.
                    },
                    tx: tx.clone(),
                    propagate_failures: false,
                })
                .await?;

            // The estimate may be off due to current limitations in confidential gas estimation.
            // Inflate the estimated gas by 20%.
            let gas = gas.saturating_add(gas.saturating_mul(20).saturating_div(100));

            tx.set_fee_gas(gas);
        }

        // Determine gas price. Currently we always use the native denomination.
        let mgp = client
            .gas_price(round, &token::Denomination::NATIVE)
            .await?;
        let fee = mgp.saturating_mul(tx.fee_gas().into());
        tx.set_fee_amount(token::BaseUnits::new(fee, token::Denomination::NATIVE));

        // Sign the transaction.
        let mut tx = tx.prepare_for_signing();
        for signer in signers {
            tx.append_sign(signer)?;
        }
        let tx = tx.finalize();

        // Submit the transaction.
        let result = tokio::time::timeout(
            Duration::from_millis(15_000), // Make sure we abort if transaction is not included.
            client.state.host.submit_tx(
                cbor::to_vec(tx),
                host::SubmitTxOpts {
                    wait: true,
                    ..Default::default()
                },
            ),
        )
        .await??
        .ok_or(anyhow!("missing result"))?;
        cbor::from_slice(&result.output).map_err(|_| anyhow!("malformed result"))
    }
}
