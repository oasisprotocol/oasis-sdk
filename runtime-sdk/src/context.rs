//! Execution context.
use core::fmt;
use std::{
    any::Any,
    collections::btree_map::{BTreeMap, Entry},
    marker::PhantomData,
    sync::Arc,
};

use io_context::Context as IoContext;
use slog::{self, o};

use oasis_core_runtime::{
    common::logger::get_logger,
    consensus,
    consensus::roothash,
    storage::mkvs,
    transaction::{context::Context as RuntimeContext, tags::Tags},
};

use crate::{
    event::Event,
    modules::core::Error,
    runtime, storage,
    types::{address::Address, message::MessageEventHookInvocation, transaction},
};

/// Transaction execution mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    ExecuteTx,
    CheckTx,
    SimulateTx,
}

const MODE_CHECK_TX: &str = "check_tx";
const MODE_EXECUTE_TX: &str = "execute_tx";
const MODE_SIMULATE_TX: &str = "simulate_tx";

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Into::<&'static str>::into(self))
    }
}

impl From<&Mode> for &'static str {
    fn from(m: &Mode) -> Self {
        match m {
            Mode::CheckTx => MODE_CHECK_TX,
            Mode::ExecuteTx => MODE_EXECUTE_TX,
            Mode::SimulateTx => MODE_SIMULATE_TX,
        }
    }
}

/// Runtime SDK context.
pub trait Context {
    /// Runtime that the context is being invoked in.
    type Runtime: runtime::Runtime;
    /// Runtime state output type.
    type Store: storage::Store;

    /// Returns a logger.
    fn get_logger(&self, module: &'static str) -> slog::Logger;

    /// Context mode.
    fn mode(&self) -> Mode;

    /// Whether the transaction is just being checked for validity.
    fn is_check_only(&self) -> bool {
        self.mode() == Mode::CheckTx
    }

    /// Whether the transaction is just being simulated.
    fn is_simulation(&self) -> bool {
        self.mode() == Mode::SimulateTx
    }

    /// Last runtime block header.
    fn runtime_header(&self) -> &roothash::Header;

    /// Results of executing the last successful runtime round.
    fn runtime_round_results(&self) -> &roothash::RoundResults;

    /// Runtime state store.
    fn runtime_state(&mut self) -> &mut Self::Store;

    /// Consensus state.
    fn consensus_state(&self) -> &consensus::state::ConsensusState;

    /// Current epoch.
    fn epoch(&self) -> consensus::beacon::EpochTime;

    /// Emits an event.
    fn emit_event<E: Event>(&mut self, event: E);

    /// Attempts to emit consensus runtime message.
    fn emit_message(
        &mut self,
        msg: roothash::Message,
        hook: MessageEventHookInvocation,
    ) -> Result<(), Error>;

    /// Attempts to emit multiple consensus runtime messages.
    fn emit_messages(
        &mut self,
        msgs: Vec<(roothash::Message, MessageEventHookInvocation)>,
    ) -> Result<(), Error> {
        for m in msgs {
            self.emit_message(m.0, m.1)?;
        }

        Ok(())
    }

    /// Returns a child io_ctx.
    fn io_ctx(&self) -> IoContext;

    /// Commit any changes made to storage, return any emitted tags and runtime messages. It
    /// consumes the transaction context.
    fn commit(self) -> (Tags, Vec<(roothash::Message, MessageEventHookInvocation)>);

    /// Fetches a value entry associated with the context.
    fn value<V: Any>(&mut self, key: &'static str) -> ContextValue<'_, V>;

    /// Executes a function in a simulation context.
    ///
    /// The simulation context collects its own messages and starts with an empty set of context
    /// values.
    fn with_simulation<F, Rs>(&mut self, f: F) -> Rs
    where
        F: FnOnce(
            RuntimeBatchContext<'_, Self::Runtime, storage::OverlayStore<&mut Self::Store>>,
        ) -> Rs;
}

/// Runtime SDK batch-wide context.
pub trait BatchContext: Context {
    /// Executes a function in a per-transaction context.
    fn with_tx<F, Rs>(&mut self, tx: transaction::Transaction, f: F) -> Rs
    where
        F: FnOnce(
            RuntimeTxContext<'_, '_, <Self as Context>::Runtime, <Self as Context>::Store>,
            transaction::Call,
        ) -> Rs;
}

/// Runtime SDK transaction context.
pub trait TxContext: Context {
    /// Transaction authentication information.
    fn tx_auth_info(&self) -> &transaction::AuthInfo;

    /// Authenticated address of the caller.
    ///
    /// In case there are multiple signers of a transaction, this will return the address
    /// corresponding to the first signer.
    fn tx_caller_address(&self) -> Address {
        self.tx_auth_info().signer_info[0].address_spec.address()
    }

    /// Fetches an entry pointing to a value associated with the transaction.
    fn tx_value<V: Any>(&mut self, key: &'static str) -> ContextValue<'_, V>;
}

/// Dispatch context for the whole batch.
pub struct RuntimeBatchContext<'a, R: runtime::Runtime, S: storage::Store> {
    mode: Mode,

    runtime_header: &'a roothash::Header,
    runtime_round_results: &'a roothash::RoundResults,
    runtime_storage: S,
    // TODO: linked consensus layer block
    consensus_state: &'a consensus::state::ConsensusState,
    epoch: consensus::beacon::EpochTime,
    io_ctx: Arc<IoContext>,
    logger: slog::Logger,

    block_tags: Tags,

    /// Maximum number of messages that can be emitted.
    max_messages: u32,
    /// Emitted messages.
    messages: Vec<(roothash::Message, MessageEventHookInvocation)>,

    /// Per-context values.
    values: BTreeMap<&'static str, Box<dyn Any>>,

    _runtime: PhantomData<R>,
}

impl<'a, R: runtime::Runtime, S: storage::Store> RuntimeBatchContext<'a, R, S> {
    /// Create a new dispatch context.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mode: Mode,
        runtime_header: &'a roothash::Header,
        runtime_round_results: &'a roothash::RoundResults,
        runtime_storage: S,
        consensus_state: &'a consensus::state::ConsensusState,
        epoch: consensus::beacon::EpochTime,
        io_ctx: Arc<IoContext>,
        max_messages: u32,
    ) -> Self {
        Self {
            mode,
            runtime_header,
            runtime_round_results,
            runtime_storage,
            consensus_state,
            epoch,
            io_ctx,
            logger: get_logger("runtime-sdk")
                .new(o!("ctx" => "dispatch", "mode" => Into::<&'static str>::into(&mode))),
            block_tags: Tags::new(),
            max_messages,
            messages: Vec::new(),
            values: BTreeMap::new(),
            _runtime: PhantomData,
        }
    }

    /// Create a new dispatch context from the low-level runtime context.
    pub(crate) fn from_runtime(
        ctx: &'a RuntimeContext<'_>,
        mkvs: &'a mut dyn mkvs::MKVS,
    ) -> RuntimeBatchContext<'a, R, storage::MKVSStore<&'a mut dyn mkvs::MKVS>> {
        let mode = if ctx.check_only {
            Mode::CheckTx
        } else {
            Mode::ExecuteTx
        };
        RuntimeBatchContext {
            mode,
            runtime_header: ctx.header,
            runtime_round_results: ctx.round_results,
            runtime_storage: storage::MKVSStore::new(ctx.io_ctx.clone(), mkvs),
            consensus_state: &ctx.consensus_state,
            epoch: ctx.epoch,
            io_ctx: ctx.io_ctx.clone(),
            logger: get_logger("runtime-sdk")
                .new(o!("ctx" => "dispatch", "mode" => Into::<&'static str>::into(&mode))),
            block_tags: Tags::new(),
            max_messages: ctx.max_messages,
            messages: Vec::new(),
            values: BTreeMap::new(),
            _runtime: PhantomData,
        }
    }
}

impl<'a, R: runtime::Runtime, S: storage::Store> Context for RuntimeBatchContext<'a, R, S> {
    type Runtime = R;
    type Store = S;

    fn get_logger(&self, module: &'static str) -> slog::Logger {
        self.logger.new(o!("sdk_module" => module))
    }

    fn mode(&self) -> Mode {
        self.mode
    }

    fn runtime_header(&self) -> &roothash::Header {
        &self.runtime_header
    }

    fn runtime_round_results(&self) -> &roothash::RoundResults {
        &self.runtime_round_results
    }

    fn runtime_state(&mut self) -> &mut Self::Store {
        &mut self.runtime_storage
    }

    fn consensus_state(&self) -> &consensus::state::ConsensusState {
        &self.consensus_state
    }

    fn epoch(&self) -> consensus::beacon::EpochTime {
        self.epoch
    }

    fn emit_event<E: Event>(&mut self, event: E) {
        self.block_tags.push(event.to_tag());
    }

    fn emit_message(
        &mut self,
        msg: roothash::Message,
        hook: MessageEventHookInvocation,
    ) -> Result<(), Error> {
        // Check against maximum number of messages that can be emitted per round.
        if self.messages.len() >= self.max_messages as usize {
            return Err(Error::OutOfMessageSlots);
        }

        self.messages.push((msg, hook));

        Ok(())
    }

    fn io_ctx(&self) -> IoContext {
        IoContext::create_child(&self.io_ctx)
    }

    fn commit(self) -> (Tags, Vec<(roothash::Message, MessageEventHookInvocation)>) {
        (self.block_tags, self.messages)
    }

    fn value<V: Any>(&mut self, key: &'static str) -> ContextValue<'_, V> {
        ContextValue::new(self.values.entry(key))
    }

    fn with_simulation<F, Rs>(&mut self, f: F) -> Rs
    where
        F: FnOnce(
            RuntimeBatchContext<'_, Self::Runtime, storage::OverlayStore<&mut Self::Store>>,
        ) -> Rs,
    {
        // Create a store wrapped by an overlay store so any state changes don't leak.
        let store = storage::OverlayStore::new(&mut self.runtime_storage);

        let sim_ctx = RuntimeBatchContext {
            mode: Mode::SimulateTx,
            runtime_header: self.runtime_header,
            runtime_round_results: self.runtime_round_results,
            runtime_storage: store,
            consensus_state: self.consensus_state,
            epoch: self.epoch,
            io_ctx: self.io_ctx.clone(),
            logger: self
                .logger
                .new(o!("ctx" => "dispatch", "mode" => MODE_SIMULATE_TX)),
            // Other than for storage, simulation has a blank slate.
            block_tags: Tags::new(),
            max_messages: self.max_messages,
            messages: Vec::new(),
            values: BTreeMap::new(),
            _runtime: PhantomData,
        };
        f(sim_ctx)
    }
}

impl<'a, R: runtime::Runtime, S: storage::Store> BatchContext for RuntimeBatchContext<'a, R, S> {
    fn with_tx<F, Rs>(&mut self, tx: transaction::Transaction, f: F) -> Rs
    where
        F: FnOnce(
            RuntimeTxContext<'_, '_, <Self as Context>::Runtime, <Self as Context>::Store>,
            transaction::Call,
        ) -> Rs,
    {
        // Create a store wrapped by an overlay store so we can either rollback or commit.
        let store = storage::OverlayStore::new(&mut self.runtime_storage);

        let tx_ctx = RuntimeTxContext {
            mode: self.mode,
            runtime_header: self.runtime_header,
            runtime_round_results: self.runtime_round_results,
            consensus_state: self.consensus_state,
            epoch: self.epoch,
            store,
            io_ctx: self.io_ctx.clone(),
            logger: self
                .logger
                .new(o!("ctx" => "transaction", "mode" => Into::<&'static str>::into(&self.mode))),
            tx_auth_info: tx.auth_info,
            tags: Tags::new(),
            // NOTE: Since a limit is enforced (which is a u32) this cast is always safe.
            max_messages: self.max_messages.saturating_sub(self.messages.len() as u32),
            messages: Vec::new(),
            values: &mut self.values,
            tx_values: BTreeMap::new(),
            _runtime: PhantomData,
        };
        f(tx_ctx, tx.call)
    }
}

/// Per-transaction/method dispatch sub-context.
pub struct RuntimeTxContext<'round, 'store, R: runtime::Runtime, S: storage::Store> {
    mode: Mode,

    runtime_header: &'round roothash::Header,
    runtime_round_results: &'round roothash::RoundResults,
    consensus_state: &'round consensus::state::ConsensusState,
    epoch: consensus::beacon::EpochTime,
    // TODO: linked consensus layer block
    store: storage::OverlayStore<&'store mut S>,
    io_ctx: Arc<IoContext>,
    logger: slog::Logger,

    /// Transaction authentication info.
    tx_auth_info: transaction::AuthInfo,

    /// Emitted tags.
    tags: Tags,

    /// Maximum number of messages that can be emitted.
    max_messages: u32,
    /// Emitted messages and respective event hooks.
    messages: Vec<(roothash::Message, MessageEventHookInvocation)>,

    /// Per-context values.
    values: &'store mut BTreeMap<&'static str, Box<dyn Any>>,

    /// Per-transaction values.
    tx_values: BTreeMap<&'static str, Box<dyn Any>>,

    _runtime: PhantomData<R>,
}

impl<'round, 'store, R: runtime::Runtime, S: storage::Store> Context
    for RuntimeTxContext<'round, 'store, R, S>
{
    type Runtime = R;
    type Store = storage::OverlayStore<&'store mut S>;

    fn get_logger(&self, module: &'static str) -> slog::Logger {
        self.logger.new(o!("sdk_module" => module))
    }

    fn mode(&self) -> Mode {
        self.mode
    }

    fn runtime_header(&self) -> &roothash::Header {
        self.runtime_header
    }

    fn runtime_round_results(&self) -> &roothash::RoundResults {
        self.runtime_round_results
    }

    fn runtime_state(&mut self) -> &mut Self::Store {
        &mut self.store
    }

    fn consensus_state(&self) -> &consensus::state::ConsensusState {
        self.consensus_state
    }

    fn epoch(&self) -> consensus::beacon::EpochTime {
        self.epoch
    }

    fn emit_event<E: Event>(&mut self, event: E) {
        self.tags.push(event.to_tag());
    }

    fn emit_message(
        &mut self,
        msg: roothash::Message,
        hook: MessageEventHookInvocation,
    ) -> Result<(), Error> {
        // Check against maximum number of messages that can be emitted per round.
        if self.messages.len() >= self.max_messages as usize {
            return Err(Error::OutOfMessageSlots);
        }

        self.messages.push((msg, hook));

        Ok(())
    }

    fn io_ctx(&self) -> IoContext {
        IoContext::create_child(&self.io_ctx)
    }

    fn commit(self) -> (Tags, Vec<(roothash::Message, MessageEventHookInvocation)>) {
        self.store.commit();
        (self.tags, self.messages)
    }

    fn value<V: Any>(&mut self, key: &'static str) -> ContextValue<'_, V> {
        ContextValue::new(self.values.entry(key))
    }

    fn with_simulation<F, Rs>(&mut self, f: F) -> Rs
    where
        F: FnOnce(
            RuntimeBatchContext<'_, Self::Runtime, storage::OverlayStore<&mut Self::Store>>,
        ) -> Rs,
    {
        // Create a store wrapped by an overlay store so any state changes don't leak.
        let store = storage::OverlayStore::new(&mut self.store);

        let sim_ctx = RuntimeBatchContext {
            mode: Mode::SimulateTx,
            runtime_header: self.runtime_header,
            runtime_round_results: self.runtime_round_results,
            runtime_storage: store,
            consensus_state: self.consensus_state,
            epoch: self.epoch,
            io_ctx: self.io_ctx.clone(),
            logger: self
                .logger
                .new(o!("ctx" => "dispatch", "mode" => MODE_SIMULATE_TX)),
            // Other than for storage, simulation has a blank slate.
            block_tags: Tags::new(),
            max_messages: self.max_messages,
            messages: Vec::new(),
            values: BTreeMap::new(),
            _runtime: PhantomData,
        };
        f(sim_ctx)
    }
}

impl<R: runtime::Runtime, S: storage::Store> TxContext for RuntimeTxContext<'_, '_, R, S> {
    fn tx_auth_info(&self) -> &transaction::AuthInfo {
        &self.tx_auth_info
    }

    fn tx_value<V: Any>(&mut self, key: &'static str) -> ContextValue<'_, V> {
        ContextValue::new(self.tx_values.entry(key))
    }
}

/// A per-context arbitrary value.
pub struct ContextValue<'a, V> {
    inner: Entry<'a, &'static str, Box<dyn Any>>,
    _value: PhantomData<V>,
}

impl<'a, V: Any> ContextValue<'a, V> {
    fn new(inner: Entry<'a, &'static str, Box<dyn Any>>) -> Self {
        Self {
            inner,
            _value: PhantomData,
        }
    }

    /// Gets a reference to the specified per-context value.
    ///
    /// # Panics
    ///
    /// Panics if the retrieved type is not the type that was stored.
    pub fn get(self) -> Option<&'a V> {
        match self.inner {
            Entry::Occupied(oe) => Some(
                oe.into_mut()
                    .downcast_ref()
                    .expect("type should stay the same"),
            ),
            _ => None,
        }
    }

    /// Gets a mutable reference to the specifeid per-context value.
    ///
    /// # Panics
    ///
    /// Panics if the retrieved type is not the type that was stored.
    pub fn get_mut(&mut self) -> Option<&mut V> {
        match &mut self.inner {
            Entry::Occupied(oe) => Some(
                oe.get_mut()
                    .downcast_mut()
                    .expect("type should stay the same"),
            ),
            _ => None,
        }
    }

    /// Sets the context value, returning a mutable reference to the set value.
    ///
    /// # Panics
    ///
    /// Panics if the retrieved type is not the type that was stored.
    pub fn set(self, value: V) -> &'a mut V {
        let value = Box::new(value);
        match self.inner {
            Entry::Occupied(mut oe) => {
                oe.insert(value);
                oe.into_mut()
            }
            Entry::Vacant(ve) => ve.insert(value),
        }
        .downcast_mut()
        .expect("type should stay the same")
    }

    /// Takes the context value, if it exists.
    ///
    /// # Panics
    ///
    /// Panics if the retrieved type is not the type that was stored.
    pub fn take(self) -> Option<V> {
        match self.inner {
            Entry::Occupied(oe) => {
                Some(*oe.remove().downcast().expect("type should stay the same"))
            }
            Entry::Vacant(_) => None,
        }
    }
}

impl<'a, V: Any + Default> ContextValue<'a, V> {
    /// Retrieves the existing value or inserts and returns the default.
    ///
    /// # Panics
    ///
    /// Panics if the retrieved type is not the type that was stored.
    pub fn or_default(self) -> &'a mut V {
        match self.inner {
            Entry::Occupied(oe) => oe.into_mut(),
            Entry::Vacant(ve) => ve.insert(Box::new(V::default())),
        }
        .downcast_mut()
        .expect("type should stay the same")
    }
}

#[cfg(test)]
#[allow(clippy::many_single_char_names)]
mod test {
    use oasis_core_runtime::{common::cbor, consensus::staking};

    use super::*;
    use crate::testing::{mock, mock::Mock};

    #[test]
    fn test_value() {
        let mut mock = Mock::default();
        let mut ctx = mock.create_ctx();

        let x = ctx.value::<u64>("module.TestKey").get();
        assert_eq!(x, None);

        ctx.value::<u64>("module.TestKey").set(42);

        let y = ctx.value::<u64>("module.TestKey").get();
        assert_eq!(y, Some(&42u64));

        let z = ctx.value::<u64>("module.TestKey").take();
        assert_eq!(z, Some(42u64));

        let y = ctx.value::<u64>("module.TestKey").get();
        assert_eq!(y, None);
    }

    #[test]
    #[should_panic]
    fn test_value_type_change() {
        let mut mock = Mock::default();
        let mut ctx = mock.create_ctx();

        ctx.value::<u64>("module.TestKey").or_default();
        ctx.value::<u32>("module.TestKey").get();
    }

    #[test]
    fn test_value_tx_context() {
        let mut mock = Mock::default();
        let mut ctx = mock.create_ctx();

        ctx.value("module.TestKey").set(42u64);

        let tx = transaction::Transaction {
            version: 1,
            call: transaction::Call {
                method: "test".to_owned(),
                body: cbor::Value::Null,
            },
            auth_info: transaction::AuthInfo {
                signer_info: vec![],
                fee: transaction::Fee {
                    amount: Default::default(),
                    gas: 1000,
                },
            },
        };
        ctx.with_tx(tx.clone(), |mut tx_ctx, _call| {
            let mut y = tx_ctx.value::<u64>("module.TestKey");
            let y = y.get_mut().unwrap();
            assert_eq!(*y, 42);
            *y = 48;

            let a = tx_ctx.tx_value::<u64>("module.TestTxKey").get();
            assert_eq!(a, None);
            tx_ctx.tx_value::<u64>("module.TestTxKey").set(65);

            let b = tx_ctx.tx_value::<u64>("module.TestTxKey").get();
            assert_eq!(b, Some(&65));

            let c = tx_ctx.tx_value::<u64>("module.TestTakeTxKey").or_default();
            *c = 67;
            let d = tx_ctx.tx_value::<u64>("module.TestTakeTxKey").take();
            assert_eq!(d, Some(67));
            let e = tx_ctx.tx_value::<u64>("module.TestTakeTxKey").get();
            assert_eq!(e, None);
        });

        let x = ctx.value::<u64>("module.TestKey").get();
        assert_eq!(x, Some(&48));

        ctx.with_tx(tx, |mut tx_ctx, _call| {
            let z = tx_ctx.value::<u64>("module.TestKey").take();
            assert_eq!(z, Some(48));

            let a = tx_ctx.tx_value::<u64>("module.TestTxKey").get();
            assert_eq!(a, None);
        });

        let y = ctx.value::<u64>("module.TestKey").get();
        assert_eq!(y, None);
    }

    #[test]
    #[should_panic]
    fn test_value_tx_context_type_change() {
        let mut mock = Mock::default();
        let mut ctx = mock.create_ctx();

        let x = ctx.value::<u64>("module.TestKey").set(0);
        *x = 42;

        let tx = transaction::Transaction {
            version: 1,
            call: transaction::Call {
                method: "test".to_owned(),
                body: cbor::Value::Null,
            },
            auth_info: transaction::AuthInfo {
                signer_info: vec![],
                fee: transaction::Fee {
                    amount: Default::default(),
                    gas: 1000,
                },
            },
        };
        ctx.with_tx(tx, |mut tx_ctx, _call| {
            // Changing the type of a key should result in a panic.
            tx_ctx.value::<Option<u32>>("module.TestKey").get();
        });
    }

    #[test]
    fn test_ctx_message_slots() {
        let mut mock = Mock::default();
        let max_messages = mock.max_messages;
        let mut ctx = mock.create_ctx();

        for _ in 0..max_messages {
            ctx.emit_message(
                roothash::Message::Staking {
                    v: 0,
                    msg: roothash::StakingMessage::Transfer(staking::Transfer::default()),
                },
                MessageEventHookInvocation::new("test".to_string(), ""),
            )
            .expect("message should be emitted");
        }

        // Another message should error.
        ctx.emit_message(
            roothash::Message::Staking {
                v: 0,
                msg: roothash::StakingMessage::Transfer(staking::Transfer::default()),
            },
            MessageEventHookInvocation::new("test".to_string(), ""),
        )
        .expect_err("message emitting should fail");
    }

    #[test]
    fn test_tx_ctx_message_slots() {
        let mut mock = Mock::default();
        let max_messages = mock.max_messages;
        let mut ctx = mock.create_ctx();

        ctx.emit_message(
            roothash::Message::Staking {
                v: 0,
                msg: roothash::StakingMessage::Transfer(staking::Transfer::default()),
            },
            MessageEventHookInvocation::new("test".to_string(), ""),
        )
        .expect("message should be emitted");

        ctx.with_tx(mock::transaction(), |mut tx_ctx, _call| {
            for _ in 0..max_messages - 1 {
                tx_ctx
                    .emit_message(
                        roothash::Message::Staking {
                            v: 0,
                            msg: roothash::StakingMessage::Transfer(staking::Transfer::default()),
                        },
                        MessageEventHookInvocation::new("test".to_string(), ""),
                    )
                    .expect("message should be emitted");
            }

            // Another message should error.
            tx_ctx
                .emit_message(
                    roothash::Message::Staking {
                        v: 0,
                        msg: roothash::StakingMessage::Transfer(staking::Transfer::default()),
                    },
                    MessageEventHookInvocation::new("test".to_string(), ""),
                )
                .expect_err("message emitting should fail");
        });
    }
}
