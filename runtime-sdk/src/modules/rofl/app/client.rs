use std::{
    collections::{BTreeMap, HashSet},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::{anyhow, Context as _, Result};
use rand::{rngs::OsRng, Rng};
use tokio::sync::{mpsc, oneshot};

use crate::{
    core::{
        common::crypto::{hash::Hash, mrae::deoxysii},
        consensus::{
            registry::{SGXConstraints, TEEHardware},
            state::{
                beacon::ImmutableState as BeaconState, registry::ImmutableState as RegistryState,
            },
        },
        enclave_rpc::{client::RpcClient, session},
        host::{self, Host as _},
        storage::mkvs,
    },
    crypto::signature::{PublicKey, Signer},
    enclave_rpc::{QueryRequest, METHOD_QUERY},
    modules::{
        self,
        accounts::types::NonceQuery,
        core::types::{CallDataPublicKeyQueryResponse, EstimateGasQuery},
    },
    state::CurrentState,
    storage::{host::new_mkvs_tree_for_round, HostStore},
    types::{
        address::{Address, SignatureAddressSpec},
        callformat, token,
        transaction::{self, CallerAddress},
    },
};

use super::{processor, App};

/// Size of various command queues.
const CMDQ_BACKLOG: usize = 16;

/// EnclaveRPC endpoint for communicating with the RONL component.
const ENCLAVE_RPC_ENDPOINT_RONL: &str = "ronl";

/// Transaction submission options.
#[derive(Clone, Debug)]
pub struct SubmitTxOpts {
    /// Optional timeout when submitting a transaction. Setting this to `None` means that the host
    /// node timeout will be used.
    pub timeout: Option<Duration>,
    /// Whether the call data should be encrypted (true by default).
    pub encrypt: bool,
    /// Whether to verify the transaction result (true by default).
    pub verify: bool,
}

impl Default for SubmitTxOpts {
    fn default() -> Self {
        Self {
            timeout: Some(Duration::from_millis(15_000)), // 15 seconds.
            encrypt: true,
            verify: true,
        }
    }
}

/// App-specific key derivation request.
#[derive(Clone, Debug, Default)]
pub struct DeriveKeyRequest {
    /// Key kind.
    pub kind: modules::rofl::types::KeyKind,
    /// Key scope.
    pub scope: modules::rofl::types::KeyScope,
    /// Key generation.
    pub generation: u64,
    /// Key identifier.
    pub key_id: Vec<u8>,
}

/// A runtime client meant for use within runtimes.
pub struct Client<A: App> {
    state: Arc<processor::State<A>>,
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
        let imp = ClientImpl::new(state.clone(), cmdq);
        let mut submission_mgr = SubmissionManager::new(imp.clone());
        submission_mgr.start();

        Self {
            state,
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

    /// Retrieves application configuration.
    pub async fn app_cfg(&self) -> Result<modules::rofl::types::AppConfig> {
        self.imp.app_cfg().await
    }

    /// Sign a given transaction, submit it and wait for block inclusion.
    ///
    /// This method supports multiple transaction signers.
    pub async fn multi_sign_and_submit_tx(
        &self,
        signers: &[Arc<dyn Signer>],
        tx: transaction::Transaction,
    ) -> Result<transaction::CallResult> {
        self.multi_sign_and_submit_tx_opts(signers, tx, SubmitTxOpts::default())
            .await
    }

    /// Sign a given transaction, submit it and wait for block inclusion.
    ///
    /// This method supports multiple transaction signers.
    pub async fn multi_sign_and_submit_tx_opts(
        &self,
        signers: &[Arc<dyn Signer>],
        tx: transaction::Transaction,
        opts: SubmitTxOpts,
    ) -> Result<transaction::CallResult> {
        self.submission_mgr
            .multi_sign_and_submit_tx(signers, tx, opts)
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

    /// Derive an application-specific key.
    pub async fn derive_key(
        &self,
        signer: Arc<dyn Signer>,
        request: DeriveKeyRequest,
    ) -> Result<modules::rofl::types::DeriveKeyResponse> {
        let tx = self.state.app.new_transaction(
            "rofl.DeriveKey",
            modules::rofl::types::DeriveKey {
                app: A::id(),
                kind: request.kind,
                scope: request.scope,
                generation: request.generation,
                key_id: request.key_id,
            },
        );
        let response = self.sign_and_submit_tx(signer, tx).await?;
        Ok(cbor::from_value(response.ok()?)?)
    }
}

impl<A> Clone for Client<A>
where
    A: App,
{
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            imp: self.imp.clone(),
            submission_mgr: self.submission_mgr.clone(),
        }
    }
}

struct ClientImpl<A: App> {
    state: Arc<processor::State<A>>,
    cmdq: mpsc::WeakSender<processor::Command>,
    latest_round: Arc<AtomicU64>,
    rpc: Arc<RpcClient>,
}

impl<A> ClientImpl<A>
where
    A: App,
{
    fn new(state: Arc<processor::State<A>>, cmdq: mpsc::WeakSender<processor::Command>) -> Self {
        Self {
            cmdq,
            latest_round: Arc::new(AtomicU64::new(0)),
            rpc: Arc::new(RpcClient::new_runtime(
                state.host.clone(),
                ENCLAVE_RPC_ENDPOINT_RONL,
                session::Builder::default()
                    .use_endorsement(true)
                    .quote_policy(None) // Forbid all until configured.
                    .local_identity(state.identity.clone())
                    .remote_enclaves(Some(HashSet::new())), // Forbid all until configured.
                2, // Maximum number of sessions (one extra for reserve).
                1, // Maximum number of sessions per peer (we only communicate with RONL).
                1, // Stale session timeout.
            )),
            state,
        }
    }

    /// Retrieve the latest known runtime round.
    async fn latest_round(&self) -> Result<u64> {
        let cmdq = self
            .cmdq
            .upgrade()
            .ok_or(anyhow!("processor has shut down"))?;
        let (tx, rx) = oneshot::channel();
        cmdq.send(processor::Command::GetLatestRound(tx)).await?;
        let round = rx.await?;
        Ok(self
            .latest_round
            .fetch_max(round, Ordering::SeqCst)
            .max(round))
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

    /// Retrieve the calldata encryption public key.
    async fn call_data_public_key(&self) -> Result<CallDataPublicKeyQueryResponse> {
        let round = self.latest_round().await?;
        self.query(round, "core.CallDataPublicKey", ()).await
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
        let tee = tokio::task::spawn_blocking(move || -> Result<_> {
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
                TEEHardware::TEEHardwareIntelSGX => Ok(ad.try_decode_tee::<SGXConstraints>()?),
                _ => Err(anyhow!("unsupported TEE platform")),
            }
        })
        .await??;

        let enclaves = HashSet::from_iter(tee.enclaves().clone());
        let quote_policy = tee.policy();
        self.rpc.update_enclaves(Some(enclaves)).await;
        self.rpc.update_quote_policy(quote_policy).await;

        let response: Vec<u8> = self
            .rpc
            .secure_call(
                METHOD_QUERY,
                QueryRequest {
                    round,
                    method: method.to_string(),
                    args: cbor::to_vec(args),
                },
                vec![],
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

    /// Retrieves application configuration.
    async fn app_cfg(&self) -> Result<modules::rofl::types::AppConfig> {
        let round = self.latest_round().await?;
        self.query(
            round,
            "rofl.App",
            modules::rofl::types::AppQuery { id: A::id() },
        )
        .await
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
            latest_round: self.latest_round.clone(),
            rpc: self.rpc.clone(),
        }
    }
}

enum Cmd {
    SubmitTx(
        Vec<Arc<dyn Signer>>,
        transaction::Transaction,
        SubmitTxOpts,
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
        opts: SubmitTxOpts,
    ) -> Result<transaction::CallResult> {
        let (ch, rx) = oneshot::channel();
        self.cmdq_tx
            .send(Cmd::SubmitTx(signers.to_vec(), tx, opts, ch))
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
                    Cmd::SubmitTx(signers, tx, opts, ch) => {
                        // Check if transaction can be executed (no conflicts with in-flight txs).
                        let signer_set =
                            HashSet::from_iter(signers.iter().map(|signer| signer.public_key()));
                        if !signer_set.is_disjoint(&pending) {
                            // Defer any non-executable commands.
                            new_queue.push(Cmd::SubmitTx(signers, tx, opts, ch));
                            continue;
                        }
                        // Include all signers in the pending set.
                        pending.extend(signer_set.iter().cloned());

                        // Execute in a separate task.
                        let client = self.client.clone();
                        let notify_tx = notify_tx.clone();

                        tokio::spawn(async move {
                            let result =
                                Self::multi_sign_and_submit_tx(client, &signers, tx, opts).await;
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
        opts: SubmitTxOpts,
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
            let mut gas = gas.saturating_add(gas.saturating_mul(20).saturating_div(100));

            // When encrypting transactions, also add the cost of calldata encryption.
            if opts.encrypt {
                let envelope_size_estimate = cbor::to_vec(callformat::CallEnvelopeX25519DeoxysII {
                    epoch: u64::MAX,
                    ..Default::default()
                })
                .len()
                .try_into()
                .unwrap();

                let params: modules::core::Parameters =
                    client.query(round, "core.Parameters", ()).await?;
                gas = gas.saturating_add(params.gas_costs.callformat_x25519_deoxysii);
                gas = gas.saturating_add(
                    params
                        .gas_costs
                        .tx_byte
                        .saturating_mul(envelope_size_estimate),
                );
            }

            tx.set_fee_gas(gas);
        }

        // Optionally perform calldata encryption.
        let meta = if opts.encrypt {
            // Obtain runtime's current ephemeral public key.
            let runtime_pk = client.call_data_public_key().await?;
            // Generate local key pair and nonce.
            let client_kp = deoxysii::generate_key_pair();
            let mut nonce = [0u8; deoxysii::NONCE_SIZE];
            OsRng.fill(&mut nonce);
            // Encrypt and encode call.
            let call = transaction::Call {
                format: transaction::CallFormat::EncryptedX25519DeoxysII,
                method: "".to_string(),
                body: cbor::to_value(callformat::CallEnvelopeX25519DeoxysII {
                    pk: client_kp.0.into(),
                    nonce,
                    epoch: runtime_pk.epoch,
                    data: deoxysii::box_seal(
                        &nonce,
                        cbor::to_vec(std::mem::take(&mut tx.call)),
                        vec![],
                        &runtime_pk.public_key.key.0,
                        &client_kp.1,
                    )?,
                }),
                ..Default::default()
            };
            tx.call = call;

            Some((runtime_pk, client_kp))
        } else {
            None
        };

        // Determine gas price. Currently we always use the native denomination.
        if tx.fee_amount().amount() == 0 {
            let mgp = client
                .gas_price(round, &token::Denomination::NATIVE)
                .await?;
            let fee = mgp.saturating_mul(tx.fee_gas().into());
            tx.set_fee_amount(token::BaseUnits::new(fee, token::Denomination::NATIVE));
        }

        // Sign the transaction.
        let mut tx = tx.prepare_for_signing();
        for signer in signers {
            tx.append_sign(signer)?;
        }
        let tx = tx.finalize();
        let raw_tx = cbor::to_vec(tx);
        let tx_hash = Hash::digest_bytes(&raw_tx);

        // Submit the transaction.
        let submit_tx_task = client.state.host.submit_tx(
            raw_tx,
            host::SubmitTxOpts {
                wait: true,
                ..Default::default()
            },
        );
        let result = if let Some(timeout) = opts.timeout {
            tokio::time::timeout(timeout, submit_tx_task).await?
        } else {
            submit_tx_task.await
        };
        let result = result?.ok_or(anyhow!("missing result"))?;

        if opts.verify {
            // TODO: Ensure consensus verifier is up to date.

            // Verify transaction inclusion and result.
            let io_tree = new_mkvs_tree_for_round(
                client.state.host.clone(),
                &client.state.consensus_verifier,
                client.state.host.get_runtime_id(),
                result.round,
                mkvs::RootType::IO,
            )
            .await?;
            // TODO: Add transaction accessors in transaction:Tree in Oasis Core.
            // TODO: Remove spawn once we have async MKVS.
            let verified_result = tokio::task::spawn_blocking(move || -> Result<_> {
                let key = [b"T", tx_hash.as_ref(), &[0x02]].concat();
                let output_artifacts = io_tree
                    .get(&key)
                    .context("failed to verify transaction result")?
                    .ok_or(anyhow!("failed to verify transaction result"))?;

                let output_artifacts: (Vec<u8>,) = cbor::from_slice(&output_artifacts)
                    .context("malformed output transaction artifacts")?;
                Ok(output_artifacts.0)
            })
            .await??;
            if result.output != verified_result {
                return Err(anyhow!("failed to verify transaction result"));
            }
        }

        // Update latest known round.
        client
            .latest_round
            .fetch_max(result.round, Ordering::SeqCst);

        // Decrypt result if it is encrypted.
        let result: transaction::CallResult =
            cbor::from_slice(&result.output).map_err(|_| anyhow!("malformed result"))?;
        match result {
            transaction::CallResult::Unknown(raw) => {
                let meta = meta.ok_or(anyhow!("unknown result but calldata was not encrypted"))?;
                let envelope: callformat::ResultEnvelopeX25519DeoxysII =
                    cbor::from_value(raw).map_err(|_| anyhow!("malformed encrypted result"))?;
                let data = deoxysii::box_open(
                    &envelope.nonce,
                    envelope.data,
                    vec![],
                    &meta.0.public_key.key.0,
                    &meta.1 .1,
                )
                .map_err(|_| anyhow!("malformed encrypted result"))?;

                cbor::from_slice(&data).map_err(|_| anyhow!("malformed encrypted result"))
            }
            _ => Ok(result),
        }
    }
}
