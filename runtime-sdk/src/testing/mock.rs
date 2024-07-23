//! Mock dispatch context for use in tests.
use std::collections::BTreeMap;

use oasis_core_runtime::{
    common::{crypto::mrae::deoxysii, namespace::Namespace, version::Version},
    consensus::{beacon, roothash, state::ConsensusState, Event},
    protocol::HostInfo,
    storage::mkvs,
    types::EventKind,
};

use crate::{
    callformat,
    context::{Context, RuntimeBatchContext},
    dispatcher,
    error::RuntimeError,
    history,
    keymanager::KeyManager,
    module::MigrationHandler,
    modules,
    runtime::Runtime,
    state::{self, CurrentState, TransactionResult},
    storage::MKVSStore,
    testing::{configmap, keymanager::MockKeyManagerClient},
    types::{self, address::SignatureAddressSpec, transaction},
};

pub struct Config;

impl modules::core::Config for Config {}

/// A mock runtime that only has the core module.
pub struct EmptyRuntime;

impl Runtime for EmptyRuntime {
    const VERSION: Version = Version::new(0, 0, 0);

    type Core = modules::core::Module<Config>;

    type Accounts = modules::accounts::Module;

    type Modules = modules::core::Module<Config>;

    fn genesis_state() -> <Self::Modules as MigrationHandler>::Genesis {
        Default::default()
    }
}

struct EmptyHistory;

impl history::HistoryHost for EmptyHistory {
    fn consensus_state_at(&self, _height: u64) -> Result<ConsensusState, history::Error> {
        Err(history::Error::FailedToFetchBlock)
    }

    fn consensus_events_at(
        &self,
        _height: u64,
        _kind: EventKind,
    ) -> Result<Vec<Event>, history::Error> {
        Err(history::Error::FailedToFetchEvents)
    }
}

/// Mock dispatch context factory.
pub struct Mock {
    pub host_info: HostInfo,
    pub runtime_header: roothash::Header,
    pub runtime_round_results: roothash::RoundResults,
    pub consensus_state: ConsensusState,
    pub history: Box<dyn history::HistoryHost>,
    pub epoch: beacon::EpochTime,

    pub max_messages: u32,
}

impl Mock {
    /// Create a new mock dispatch context.
    pub fn create_ctx(&mut self) -> RuntimeBatchContext<'_, EmptyRuntime> {
        self.create_ctx_for_runtime(false)
    }

    /// Create a new mock dispatch context.
    pub fn create_ctx_for_runtime<R: Runtime>(
        &mut self,
        confidential: bool,
    ) -> RuntimeBatchContext<'_, R> {
        RuntimeBatchContext::new(
            &self.host_info,
            if confidential {
                Some(Box::new(MockKeyManagerClient::new()) as Box<dyn KeyManager>)
            } else {
                None
            },
            &self.runtime_header,
            &self.runtime_round_results,
            &self.consensus_state,
            &self.history,
            self.epoch,
            self.max_messages,
        )
    }

    /// Create an instance with the given local configuration.
    pub fn with_local_config(local_config: BTreeMap<String, cbor::Value>) -> Self {
        // Ensure a current state is always available during tests. Note that one can always use a
        // different store by calling `CurrentState::enter` explicitly.
        CurrentState::init_local_fallback();

        let consensus_tree = mkvs::Tree::builder()
            .with_root_type(mkvs::RootType::State)
            .build(Box::new(mkvs::sync::NoopReadSyncer));

        Self {
            host_info: HostInfo {
                runtime_id: Namespace::default(),
                consensus_backend: "mock".to_string(),
                consensus_protocol_version: Version::default(),
                consensus_chain_context: "test".to_string(),
                local_config,
            },
            runtime_header: roothash::Header::default(),
            runtime_round_results: roothash::RoundResults::default(),
            consensus_state: ConsensusState::new(1, consensus_tree),
            history: Box::new(EmptyHistory),
            epoch: 1,
            max_messages: 32,
        }
    }
}

impl Default for Mock {
    fn default() -> Self {
        let local_config_for_tests = configmap! {
            // Allow expensive gas estimation and expensive queries so they can be tested.
            "estimate_gas_by_simulating_contracts" => true,
            "allowed_queries" => vec![
                configmap! {"all_expensive" => true}
            ],
        };
        Self::with_local_config(local_config_for_tests)
    }
}

/// Create an empty MKVS store.
pub fn empty_store() -> MKVSStore<mkvs::OverlayTree<mkvs::Tree>> {
    let root = mkvs::OverlayTree::new(
        mkvs::Tree::builder()
            .with_root_type(mkvs::RootType::State)
            .build(Box::new(mkvs::sync::NoopReadSyncer)),
    );
    MKVSStore::new(root)
}

/// Create a new mock transaction.
pub fn transaction() -> transaction::Transaction {
    transaction::Transaction {
        version: 1,
        call: transaction::Call {
            format: transaction::CallFormat::Plain,
            method: "mock".to_owned(),
            body: cbor::Value::Simple(cbor::SimpleValue::NullValue),
            ..Default::default()
        },
        auth_info: transaction::AuthInfo {
            signer_info: vec![],
            fee: transaction::Fee {
                amount: Default::default(),
                gas: 1_000_000,
                consensus_messages: 32,
                ..Default::default()
            },
            ..Default::default()
        },
    }
}

/// Options that can be used during mock signer calls.
#[derive(Clone, Debug)]
pub struct CallOptions {
    /// Transaction fee.
    pub fee: transaction::Fee,
    /// Should the call be encrypted.
    pub encrypted: bool,
}

impl Default for CallOptions {
    fn default() -> Self {
        Self {
            fee: transaction::Fee {
                amount: Default::default(),
                gas: 1_000_000,
                consensus_messages: 0,
                ..Default::default()
            },
            encrypted: false,
        }
    }
}

/// A mock signer for use during tests.
pub struct Signer {
    nonce: u64,
    sigspec: SignatureAddressSpec,
}

impl Signer {
    /// Create a new mock signer using the given nonce and signature spec.
    pub fn new(nonce: u64, sigspec: SignatureAddressSpec) -> Self {
        Self { nonce, sigspec }
    }

    /// Address specification for this signer.
    pub fn sigspec(&self) -> &SignatureAddressSpec {
        &self.sigspec
    }

    /// Dispatch a call to the given method.
    pub fn call<C, B>(&mut self, ctx: &C, method: &str, body: B) -> dispatcher::DispatchResult
    where
        C: Context,
        B: cbor::Encode,
    {
        self.call_opts(ctx, method, body, Default::default())
    }

    /// Dispatch a call to the given method with the given options.
    pub fn call_opts<C, B>(
        &mut self,
        ctx: &C,
        method: &str,
        body: B,
        opts: CallOptions,
    ) -> dispatcher::DispatchResult
    where
        C: Context,
        B: cbor::Encode,
    {
        let mut call = transaction::Call {
            format: transaction::CallFormat::Plain,
            method: method.to_owned(),
            body: cbor::to_value(body),
            ..Default::default()
        };
        if opts.encrypted {
            let key_pair = deoxysii::generate_key_pair();
            let nonce = [0u8; deoxysii::NONCE_SIZE];
            let km = ctx.key_manager().unwrap();
            let epoch = ctx.epoch();
            let runtime_keypair = km
                .get_or_create_ephemeral_keys(callformat::get_key_pair_id(epoch), epoch)
                .unwrap();
            let runtime_pk = runtime_keypair.input_keypair.pk;
            call = transaction::Call {
                format: transaction::CallFormat::EncryptedX25519DeoxysII,
                method: "".to_owned(),
                body: cbor::to_value(types::callformat::CallEnvelopeX25519DeoxysII {
                    pk: key_pair.0.into(),
                    nonce,
                    epoch,
                    data: deoxysii::box_seal(
                        &nonce,
                        cbor::to_vec(call),
                        vec![],
                        &runtime_pk.0,
                        &key_pair.1,
                    )
                    .unwrap(),
                }),
                ..Default::default()
            }
        };
        let tx = transaction::Transaction {
            version: 1,
            call,
            auth_info: transaction::AuthInfo {
                signer_info: vec![transaction::SignerInfo::new_sigspec(
                    self.sigspec.clone(),
                    self.nonce,
                )],
                fee: opts.fee,
                ..Default::default()
            },
        };

        let result = dispatcher::Dispatcher::<C::Runtime>::dispatch_tx(ctx, 1024, tx, 0)
            .expect("dispatch should work");

        // Increment the nonce.
        self.nonce += 1;

        result
    }

    /// Dispatch a query to the given method.
    pub fn query<C, A, R>(&self, ctx: &C, method: &str, args: A) -> Result<R, RuntimeError>
    where
        C: Context,
        A: cbor::Encode,
        R: cbor::Decode,
    {
        let result = CurrentState::with_transaction_opts(
            state::Options::new().with_mode(state::Mode::Check),
            || {
                let result = dispatcher::Dispatcher::<C::Runtime>::dispatch_query(
                    ctx,
                    method,
                    cbor::to_vec(args),
                );

                TransactionResult::Rollback(result)
            },
        )?;
        Ok(cbor::from_slice(&result).expect("result should decode correctly"))
    }
}
