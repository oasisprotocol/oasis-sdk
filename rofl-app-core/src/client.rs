use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::{anyhow, Context as _, Result};
use rand::{rngs::SysRng, TryRng};
use tokio::sync::{mpsc, oneshot};

use oasis_runtime_sdk::{
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
        types,
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
            timeout: Some(Duration::from_millis(60_000)), // 60 seconds.
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

impl Cmd {
    /// Public keys of the command's signers.
    fn public_keys(&self) -> Vec<PublicKey> {
        match self {
            Cmd::SubmitTx(signers, ..) => {
                signers.iter().map(|signer| signer.public_key()).collect()
            }
        }
    }
}

/// Notification from a submission task to the submission manager task.
enum Notify {
    /// The first submission of a session read the signer's nonce from chain state.
    Seeded(PublicKey, u64),
    /// A submission takes the next nonce of its signer's session.
    TakeNonce(PublicKey, oneshot::Sender<u64>),
    /// A submission has finished.
    Finished(Vec<PublicKey>, Outcome),
}

/// Where a submission takes its nonces from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NonceSource {
    /// Read the nonces of all signers from chain state.
    Chain,
    /// Read the single signer's nonce from chain state and start its session with it.
    Seed,
    /// Take the single signer's nonce from its session.
    Session,
}

/// How a submission ended, as far as nonces are concerned.
#[derive(Clone)]
enum Outcome {
    /// The submission failed before taking a nonce.
    Unprepared,
    /// The transaction landed, or it will not land and its nonces came from chain state.
    Done,
    /// The single signer's nonce is unused, as the transaction never reached the node or the node
    /// rejected it in its check.
    Unused(u64),
    /// The transaction may still land with these nonces, one per signer.
    Unknown(Vec<u64>),
}

/// Nonce session of a signer with running submissions.
#[derive(Default)]
struct Session {
    /// Number of running submissions.
    running: usize,
    /// Whether a submission has yet to take its nonce. Later ones wait, so nonces are taken in
    /// arrival order.
    preparing: bool,
    /// Next fresh nonce. Unset until the first submission has read it from chain state, and while
    /// earlier transactions in doubt can still be pending.
    next: Option<u64>,
    /// Unused nonces, taken before fresh ones.
    unused: BTreeSet<u64>,
}

/// Nonce sessions of all signers with running submissions.
///
/// A session starts when an idle signer reads its nonce from chain state and ends when the signer
/// has nothing running. Within a session, nonces are handed out without waiting for inclusion. Once
/// a transaction may or may not land, nothing new starts until the session ends, so the next
/// submission reads the nonce from chain state again. Until the chain nonce passes the nonces of
/// such transactions, sessions of the signer run a single submission, as those transactions can
/// still be pending.
struct Sessions {
    max_in_flight: usize,
    sessions: HashMap<PublicKey, Session>,
    /// Per signer, one above the highest nonce of its transactions that may still land.
    in_doubt_until: HashMap<PublicKey, u64>,
}

impl Sessions {
    fn new(max_in_flight: usize) -> Self {
        Self {
            max_in_flight,
            sessions: HashMap::new(),
            in_doubt_until: HashMap::new(),
        }
    }

    /// Start queued submissions in arrival order. A waiting submission holds back later ones that
    /// share a signer with it. Returns the waiting submissions and the started ones.
    fn start_ready<T>(
        &mut self,
        queue: Vec<T>,
        public_keys: impl Fn(&T) -> Vec<PublicKey>,
    ) -> (Vec<T>, Vec<(T, NonceSource)>) {
        let mut waiting = Vec::new();
        let mut started = Vec::new();
        let mut held = HashSet::new();
        for item in queue {
            let pks = public_keys(&item);
            if pks.iter().all(|pk| !held.contains(pk)) {
                if let Some(source) = self.start(&pks) {
                    started.push((item, source));
                    continue;
                }
            }
            held.extend(pks);
            waiting.push(item);
        }
        (waiting, started)
    }

    /// Start a submission if its signers' sessions allow it.
    fn start(&mut self, pks: &[PublicKey]) -> Option<NonceSource> {
        if let [pk] = pks {
            let Some(session) = self.sessions.get_mut(pk) else {
                self.sessions.insert(
                    pk.clone(),
                    Session {
                        running: 1,
                        ..Default::default()
                    },
                );
                return Some(NonceSource::Seed);
            };
            if session.preparing
                || session.next.is_none()
                || session.running >= self.max_in_flight
                || self.in_doubt_until.contains_key(pk)
            {
                return None;
            }
            session.running += 1;
            session.preparing = true;
            return Some(NonceSource::Session);
        }

        // Submissions with multiple signers run alone.
        if pks.iter().any(|pk| self.sessions.contains_key(pk)) {
            return None;
        }
        for pk in pks {
            self.sessions.entry(pk.clone()).or_default().running += 1;
        }
        Some(NonceSource::Chain)
    }

    /// Record the nonce that the first submission of a session read from chain state.
    fn seeded(&mut self, pk: &PublicKey, nonce: u64) {
        if matches!(self.in_doubt_until.get(pk), Some(&until) if nonce < until) {
            return;
        }
        self.in_doubt_until.remove(pk);
        if let Some(session) = self.sessions.get_mut(pk) {
            session.next = nonce.checked_add(1);
        }
    }

    /// Take the next nonce of a seeded session.
    fn take_nonce(&mut self, pk: &PublicKey) -> Option<u64> {
        let session = self.sessions.get_mut(pk)?;
        session.preparing = false;
        if let Some(nonce) = session.unused.pop_first() {
            return Some(nonce);
        }
        let nonce = session.next?;
        session.next = nonce.checked_add(1);
        Some(nonce)
    }

    /// Record a finished submission.
    fn finished(&mut self, pks: &[PublicKey], outcome: Outcome) {
        for (i, pk) in pks.iter().enumerate() {
            let Some(session) = self.sessions.get_mut(pk) else {
                continue;
            };
            session.running -= 1;
            match &outcome {
                Outcome::Unprepared => session.preparing = false,
                Outcome::Done => {}
                Outcome::Unused(nonce) => {
                    session.unused.insert(*nonce);
                }
                Outcome::Unknown(nonces) => {
                    let until = self.in_doubt_until.entry(pk.clone()).or_default();
                    *until = (*until).max(nonces[i].saturating_add(1));
                }
            }
            if session.running == 0 {
                self.sessions.remove(pk);
            }
        }
    }
}

/// Whether the node rejected a transaction in its check for a reason other than its nonce. Such a
/// transaction is neither pooled nor gossiped.
fn is_check_rejection(err: &types::Error) -> bool {
    // Oasis Core reports a failed check as runtime/client error 5 with the runtime error in the
    // message, and the runtime reports an invalid nonce as core error 4.
    err.module == "runtime/client"
        && err.code == 5
        && !err.message.contains("module: core code: 4 message:")
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
        let (notify_tx, mut notify_rx) = mpsc::channel::<Notify>(CMDQ_BACKLOG);
        let mut queue: Vec<Cmd> = Vec::new();
        let mut sessions = Sessions::new(A::max_in_flight_txs_per_signer());

        loop {
            tokio::select! {
                // Process incoming commands.
                Some(cmd) = self.cmdq_rx.recv() => queue.push(cmd),

                // Process incoming notifications.
                Some(notify) = notify_rx.recv() => match notify {
                    Notify::Seeded(pk, nonce) => sessions.seeded(&pk, nonce),
                    Notify::TakeNonce(pk, ch) => {
                        if let Some(nonce) = sessions.take_nonce(&pk) {
                            let _ = ch.send(nonce);
                        }
                    }
                    Notify::Finished(pks, outcome) => sessions.finished(&pks, outcome),
                },

                else => break,
            }

            // Start everything in the queue that the signers' sessions allow.
            let (waiting, started) = sessions.start_ready(queue, Cmd::public_keys);
            queue = waiting;
            for (Cmd::SubmitTx(signers, tx, opts, ch), source) in started {
                // Execute in a separate task.
                let client = self.client.clone();
                let notify_tx = notify_tx.clone();

                tokio::spawn(async move {
                    let pks = signers.iter().map(|signer| signer.public_key()).collect();
                    let mut outcome = Outcome::Unprepared;
                    let result = Self::multi_sign_and_submit_tx(
                        client,
                        &signers,
                        tx,
                        opts,
                        source,
                        &notify_tx,
                        &mut outcome,
                    )
                    .await;

                    // Notify the submission manager task before replying, so that a follow-up
                    // submission does not wait for this one.
                    let _ = notify_tx.send(Notify::Finished(pks, outcome)).await;
                    let _ = ch.send(result);
                });
            }
        }
    }

    /// Sign a given transaction, submit it and wait for block inclusion.
    ///
    /// Records in `outcome` whether the transaction may still land.
    async fn multi_sign_and_submit_tx(
        client: ClientImpl<A>,
        signers: &[Arc<dyn Signer>],
        mut tx: transaction::Transaction,
        opts: SubmitTxOpts,
        source: NonceSource,
        notify_tx: &mpsc::Sender<Notify>,
        outcome: &mut Outcome,
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

        // Perform gas estimation with all signer infos added as otherwise we may underestimate the
        // amount of gas needed. Nonces are resolved last, so use placeholders of maximum size.
        if tx.fee_gas() == 0 {
            let mut estimate_tx = tx.clone();
            for (_, sigspec) in &addresses {
                estimate_tx.append_auth_signature(sigspec.clone(), u64::MAX);
            }
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
                    tx: estimate_tx,
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
            SysRng.try_fill_bytes(&mut nonce).unwrap();
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
            tx.set_fee_amount(token::BaseUnits::native(fee));
        }

        // Resolve account nonces last, so that a failure above leaves no nonce unused.
        let nonce = match source {
            NonceSource::Chain => None,
            NonceSource::Seed => {
                let nonce = client.account_nonce(round, addresses[0].0).await?;
                let _ = notify_tx
                    .send(Notify::Seeded(signers[0].public_key(), nonce))
                    .await;
                Some(nonce)
            }
            NonceSource::Session => {
                let (ch, rx) = oneshot::channel();
                notify_tx
                    .send(Notify::TakeNonce(signers[0].public_key(), ch))
                    .await?;
                Some(rx.await?)
            }
        };
        let mut nonces = Vec::with_capacity(addresses.len());
        for (address, sigspec) in &addresses {
            let nonce = match nonce {
                Some(nonce) => nonce,
                None => client.account_nonce(round, *address).await?,
            };

            tx.append_auth_signature(sigspec.clone(), nonce);
            nonces.push(nonce);
        }

        // The nonce stays unused until the transaction reaches the node.
        let unused = nonce.map_or(Outcome::Done, Outcome::Unused);
        *outcome = unused.clone();

        // Sign the transaction.
        let mut tx = tx.prepare_for_signing();
        for signer in signers {
            tx.append_sign(signer)?;
        }
        let tx = tx.finalize();
        let raw_tx = cbor::to_vec(tx);
        let tx_hash = Hash::digest_bytes(&raw_tx);

        // Submit the transaction.
        *outcome = Outcome::Unknown(nonces);
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
        if let Err(host::Error::Host(err)) = &result {
            if is_check_rejection(err) {
                *outcome = unused;
            }
        }
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
        *outcome = Outcome::Done;

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

#[cfg(test)]
mod test {
    use oasis_runtime_sdk::testing::keys;

    use super::*;

    #[test]
    fn test_max_one_reads_every_nonce_from_chain() {
        let alice = &[keys::alice::pk()];
        let mut sessions = Sessions::new(1);

        assert_eq!(sessions.start(alice), Some(NonceSource::Seed));
        assert_eq!(sessions.start(alice), None);
        sessions.seeded(&alice[0], 5);
        assert_eq!(sessions.start(alice), None);
        sessions.finished(alice, Outcome::Done);
        assert_eq!(sessions.start(alice), Some(NonceSource::Seed));
    }

    #[test]
    fn test_session_window() {
        let alice = &[keys::alice::pk()];
        let mut sessions = Sessions::new(10);

        assert_eq!(sessions.start(alice), Some(NonceSource::Seed));
        // Nothing else starts until the first submission has read the nonce.
        assert_eq!(sessions.start(alice), None);
        sessions.seeded(&alice[0], 5);
        for nonce in 6..15 {
            assert_eq!(sessions.start(alice), Some(NonceSource::Session));
            assert_eq!(sessions.take_nonce(&alice[0]), Some(nonce));
        }
        // The 11th waits for one of the 10 to finish.
        assert_eq!(sessions.start(alice), None);
        sessions.finished(alice, Outcome::Done);
        assert_eq!(sessions.start(alice), Some(NonceSource::Session));
        assert_eq!(sessions.take_nonce(&alice[0]), Some(15));
    }

    #[test]
    fn test_unused_nonce_is_taken_first() {
        let alice = &[keys::alice::pk()];
        let mut sessions = Sessions::new(10);

        assert_eq!(sessions.start(alice), Some(NonceSource::Seed));
        sessions.seeded(&alice[0], 5);
        for nonce in 6..9 {
            assert_eq!(sessions.start(alice), Some(NonceSource::Session));
            assert_eq!(sessions.take_nonce(&alice[0]), Some(nonce));
        }
        sessions.finished(alice, Outcome::Unused(6));
        assert_eq!(sessions.start(alice), Some(NonceSource::Session));
        assert_eq!(sessions.take_nonce(&alice[0]), Some(6));
        assert_eq!(sessions.start(alice), Some(NonceSource::Session));
        assert_eq!(sessions.take_nonce(&alice[0]), Some(9));
    }

    #[test]
    fn test_unknown_outcome_drains_session() {
        let alice = &[keys::alice::pk()];
        let mut sessions = Sessions::new(10);

        assert_eq!(sessions.start(alice), Some(NonceSource::Seed));
        sessions.seeded(&alice[0], 5);
        assert_eq!(sessions.start(alice), Some(NonceSource::Session));
        assert_eq!(sessions.take_nonce(&alice[0]), Some(6));
        assert_eq!(sessions.start(alice), Some(NonceSource::Session));
        sessions.finished(alice, Outcome::Unknown(vec![6]));

        // Nothing new starts, but a running submission still gets its nonce.
        assert_eq!(sessions.start(alice), None);
        assert_eq!(sessions.take_nonce(&alice[0]), Some(7));
        sessions.finished(alice, Outcome::Unused(7));
        assert_eq!(sessions.start(alice), None);

        // Once nothing runs, the next submission reads the nonce from chain state.
        sessions.finished(alice, Outcome::Done);
        assert_eq!(sessions.start(alice), Some(NonceSource::Seed));

        // It runs alone until the chain nonce passes the nonce in doubt.
        sessions.seeded(&alice[0], 6);
        assert_eq!(sessions.start(alice), None);
        sessions.finished(alice, Outcome::Done);
        assert_eq!(sessions.start(alice), Some(NonceSource::Seed));
        sessions.seeded(&alice[0], 7);
        assert_eq!(sessions.start(alice), Some(NonceSource::Session));
    }

    #[test]
    fn test_doubt_mark_keeps_highest_nonce() {
        let alice = &[keys::alice::pk()];
        let mut sessions = Sessions::new(10);

        assert_eq!(sessions.start(alice), Some(NonceSource::Seed));
        sessions.seeded(&alice[0], 5);
        for nonce in 6..8 {
            assert_eq!(sessions.start(alice), Some(NonceSource::Session));
            assert_eq!(sessions.take_nonce(&alice[0]), Some(nonce));
        }
        // A lower nonce in doubt does not lower the mark.
        sessions.finished(alice, Outcome::Unknown(vec![7]));
        sessions.finished(alice, Outcome::Unknown(vec![6]));
        sessions.finished(alice, Outcome::Done);

        assert_eq!(sessions.start(alice), Some(NonceSource::Seed));
        sessions.seeded(&alice[0], 7);
        assert_eq!(sessions.start(alice), None);
    }

    #[test]
    fn test_unknown_registration_marks_each_signer() {
        let (rak, app) = (keys::alice::pk(), keys::bob::pk());
        let (registration, app_only) = ([rak.clone(), app.clone()], [app.clone()]);
        let mut sessions = Sessions::new(10);

        assert_eq!(sessions.start(&registration), Some(NonceSource::Chain));
        sessions.finished(&registration, Outcome::Unknown(vec![3, 5]));

        // Each signer runs alone until the chain nonce passes its nonce in doubt.
        for (pks, nonce) in [([rak.clone()], 3), ([app.clone()], 5)] {
            assert_eq!(sessions.start(&pks), Some(NonceSource::Seed));
            sessions.seeded(&pks[0], nonce);
            assert_eq!(sessions.start(&pks), None);
            sessions.finished(&pks, Outcome::Done);
        }

        // Registration does not wait for the doubt to clear.
        assert_eq!(sessions.start(&registration), Some(NonceSource::Chain));
        sessions.finished(&registration, Outcome::Done);

        assert_eq!(sessions.start(&app_only), Some(NonceSource::Seed));
        sessions.seeded(&app, 6);
        assert_eq!(sessions.start(&app_only), Some(NonceSource::Session));
    }

    #[test]
    fn test_nonces_are_taken_in_arrival_order() {
        let alice = &[keys::alice::pk()];
        let mut sessions = Sessions::new(10);

        assert_eq!(sessions.start(alice), Some(NonceSource::Seed));
        sessions.seeded(&alice[0], 5);
        let (waiting, started) = sessions.start_ready(vec!["first", "second"], |_| alice.to_vec());
        assert_eq!(started, vec![("first", NonceSource::Session)]);
        assert_eq!(waiting, vec!["second"]);

        // The second starts once the first has taken its nonce.
        assert_eq!(sessions.take_nonce(&alice[0]), Some(6));
        let (waiting, started) = sessions.start_ready(waiting, |_| alice.to_vec());
        assert_eq!(started, vec![("second", NonceSource::Session)]);
        assert!(waiting.is_empty());

        // A submission that fails before taking its nonce lets the next one start.
        sessions.finished(alice, Outcome::Unprepared);
        assert_eq!(sessions.start(alice), Some(NonceSource::Session));
        assert_eq!(sessions.take_nonce(&alice[0]), Some(7));
    }

    #[test]
    fn test_failed_seed_ends_session() {
        let alice = &[keys::alice::pk()];
        let mut sessions = Sessions::new(10);

        assert_eq!(sessions.start(alice), Some(NonceSource::Seed));
        sessions.finished(alice, Outcome::Unprepared);
        assert_eq!(sessions.start(alice), Some(NonceSource::Seed));
    }

    #[test]
    fn test_multiple_signers_run_alone() {
        let (alice, bob) = (&[keys::alice::pk()], &[keys::bob::pk()]);
        let both = &[alice[0].clone(), bob[0].clone()];
        let mut sessions = Sessions::new(10);

        assert_eq!(sessions.start(both), Some(NonceSource::Chain));
        assert_eq!(sessions.start(alice), None);
        assert_eq!(sessions.start(bob), None);
        sessions.finished(both, Outcome::Done);

        assert_eq!(sessions.start(alice), Some(NonceSource::Seed));
        assert_eq!(sessions.start(both), None);
        assert_eq!(sessions.start(bob), Some(NonceSource::Seed));
    }

    #[test]
    fn test_waiting_submission_holds_back_later_ones() {
        // Registration (RAK and app key) while the app key is busy.
        let (rak, app, other) = (keys::alice::pk(), keys::bob::pk(), keys::charlie::pk());
        let app_only = [app.clone()];
        let mut sessions = Sessions::new(10);

        assert_eq!(sessions.start(&app_only), Some(NonceSource::Seed));
        sessions.seeded(&app, 0);
        assert_eq!(sessions.start(&app_only), Some(NonceSource::Session));

        let queue = vec![
            vec![rak.clone(), app.clone()],
            vec![app.clone()],
            vec![rak.clone()],
            vec![other.clone()],
        ];
        let (waiting, started) = sessions.start_ready(queue, Vec::clone);
        assert_eq!(started, vec![(vec![other.clone()], NonceSource::Seed)]);
        assert_eq!(
            waiting,
            vec![
                vec![rak.clone(), app.clone()],
                vec![app.clone()],
                vec![rak.clone()]
            ]
        );

        // Registration runs once the app key's submissions finish.
        sessions.finished(&app_only, Outcome::Done);
        sessions.finished(&app_only, Outcome::Done);
        let (waiting, started) = sessions.start_ready(waiting, Vec::clone);
        assert_eq!(
            started,
            vec![(vec![rak.clone(), app.clone()], NonceSource::Chain)]
        );
        assert_eq!(waiting, vec![vec![app.clone()], vec![rak.clone()]]);
    }

    #[test]
    fn test_is_check_rejection() {
        let err = |module: &str, code, message: &str| types::Error {
            module: module.to_string(),
            code,
            message: message.to_string(),
        };

        assert!(is_check_rejection(&err(
            "runtime/client",
            5,
            "client: transaction check failed: runtime error: module: evm code: 8 message: reverted: AAAA",
        )));
        assert!(is_check_rejection(&err(
            "runtime/client",
            5,
            "client: transaction check failed: runtime error: module: core code: 5 message: insufficient balance to pay fees",
        )));
        assert!(!is_check_rejection(&err(
            "runtime/client",
            5,
            "client: transaction check failed: runtime error: module: core code: 4 message: invalid nonce",
        )));
        assert!(!is_check_rejection(&err(
            "runtime/client",
            4,
            "client: not finished initial sync",
        )));
        assert!(!is_check_rejection(&err("", 1, "duplicate transaction")));
    }

    /// Deterministic random number generator for the model test.
    struct Rng(u64);

    impl Rng {
        fn below(&mut self, n: u64) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0 % n
        }
    }

    #[test]
    fn test_model() {
        const MAX_IN_FLIGHT: u64 = 10;
        const MAX_FUTURE_DELTA: u64 = 9;
        const ARRIVAL_STEPS: usize = 100_000;

        enum State {
            Preparing(NonceSource),
            Submitted(u64),
        }

        let (app, rak) = (keys::alice::pk(), keys::bob::pk());
        let mut rng = Rng(2551);
        let mut sessions = Sessions::new(MAX_IN_FLIGHT as usize);
        let mut queue: Vec<Vec<PublicKey>> = Vec::new();
        let mut tasks: BTreeMap<usize, (Vec<PublicKey>, State)> = BTreeMap::new();
        let mut next_id = 0;
        // The app key's account nonce and its pooled transactions by nonce.
        let mut chain = 0;
        let mut pool: BTreeMap<u64, usize> = BTreeMap::new();
        let mut landed: HashSet<usize> = HashSet::new();
        let (mut done, mut max_submitted) = (0, 0);

        for step in 0.. {
            if step >= ARRIVAL_STEPS && queue.is_empty() && tasks.is_empty() {
                break;
            }
            assert!(step < 2 * ARRIVAL_STEPS, "submissions stalled");

            match rng.below(20) {
                0 if step < ARRIVAL_STEPS && rng.below(10) == 0 => {
                    // A burst of submissions, where registration is signed by the RAK and the
                    // app key.
                    for _ in 0..=rng.below(15) {
                        let pks = if rng.below(20) == 0 {
                            vec![rak.clone(), app.clone()]
                        } else {
                            vec![app.clone()]
                        };
                        queue.push(pks);
                    }
                }
                1..=3 => {
                    if let Some(id) = pool.remove(&chain) {
                        landed.insert(id);
                        chain += 1;
                    }
                }
                5 if rng.below(100) == 0 => {
                    // The node loses a pooled transaction.
                    let nonce = chain + rng.below(MAX_IN_FLIGHT);
                    pool.remove(&nonce);
                }
                _ if !tasks.is_empty() => {
                    let id = *tasks
                        .keys()
                        .nth(rng.below(tasks.len() as u64) as usize)
                        .unwrap();
                    let signers = tasks[&id].0.len();
                    let outcome = match tasks[&id].1 {
                        State::Preparing(_) if rng.below(10) == 0 => {
                            // Preparation failed before taking a nonce.
                            Some(Outcome::Unprepared)
                        }
                        State::Preparing(source) => {
                            assert!(
                                tasks
                                    .range(..id)
                                    .all(|(_, (_, state))| !matches!(state, State::Preparing(_))),
                                "nonce taken out of arrival order"
                            );
                            let nonce = match source {
                                NonceSource::Chain => chain,
                                NonceSource::Seed => {
                                    sessions.seeded(&app, chain);
                                    chain
                                }
                                NonceSource::Session => sessions.take_nonce(&app).unwrap(),
                            };
                            assert!(nonce < chain + MAX_IN_FLIGHT, "nonce beyond the window");
                            if source == NonceSource::Session {
                                assert!(
                                    pool.get(&nonce).is_none_or(|id| tasks.contains_key(id)),
                                    "nonce of a transaction that can still land"
                                );
                            }
                            assert!(
                                tasks
                                    .values()
                                    .all(|(_, state)| !matches!(state, State::Submitted(n) if *n == nonce)),
                                "nonce shared by running submissions"
                            );
                            let unused = match source {
                                NonceSource::Chain => Outcome::Done,
                                _ => Outcome::Unused(nonce),
                            };

                            if nonce < chain || nonce > chain + MAX_FUTURE_DELTA {
                                // The node rejects an invalid nonce in its check.
                                Some(Outcome::Unknown(vec![nonce; signers]))
                            } else if rng.below(20) == 0 {
                                // The node rejects the transaction in its check.
                                Some(unused)
                            } else {
                                // A transaction with a pooled nonce is dropped as an underpriced
                                // replacement, but reported as accepted.
                                pool.entry(nonce).or_insert(id);
                                tasks.get_mut(&id).unwrap().1 = State::Submitted(nonce);
                                let submitted = tasks
                                    .values()
                                    .filter(|(_, state)| matches!(state, State::Submitted(_)))
                                    .count();
                                max_submitted = max_submitted.max(submitted);
                                None
                            }
                        }
                        State::Submitted(_) if landed.remove(&id) => {
                            done += 1;
                            Some(Outcome::Done)
                        }
                        // The submission times out.
                        State::Submitted(nonce) if rng.below(50) == 0 => {
                            Some(Outcome::Unknown(vec![nonce; signers]))
                        }
                        State::Submitted(_) => None,
                    };
                    if let Some(outcome) = outcome {
                        let (pks, _) = tasks.remove(&id).unwrap();
                        sessions.finished(&pks, outcome);
                    }
                }
                _ => {}
            }

            let (waiting, started) = sessions.start_ready(std::mem::take(&mut queue), Vec::clone);
            queue = waiting;
            for (pks, source) in started {
                tasks.insert(next_id, (pks, State::Preparing(source)));
                next_id += 1;
            }
        }

        assert!(sessions.sessions.is_empty());
        assert_eq!(max_submitted, MAX_IN_FLIGHT as usize);
        assert!(done > next_id / 2, "too few landed: {done} of {next_id}");
    }
}
