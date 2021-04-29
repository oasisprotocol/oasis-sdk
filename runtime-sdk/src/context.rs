//! Execution context.
use core::fmt;
use std::{any::Any, collections::BTreeMap, sync::Arc};

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
    module::MethodRegistry,
    modules::core::Error,
    storage,
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
    /// Runtime state output type.
    type S: storage::Store;

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
    fn runtime_state(&mut self) -> &mut Self::S;

    /// Consensus state.
    fn consensus_state(&self) -> &consensus::state::ConsensusState;

    /// Transaction authentication information.
    ///
    /// Only present if this is a transaction processing context.
    fn tx_auth_info(&self) -> Option<&transaction::AuthInfo>;

    /// Authenticated address of the caller.
    ///
    /// In case there are multiple signers of a transaction, this will return the address
    /// corresponding to the first signer.
    ///
    /// Only present if this is a transaction processing context.
    fn tx_caller_address(&self) -> Option<Address> {
        self.tx_auth_info()
            .map(|info| Address::from_pk(&info.signer_info[0].public_key))
    }

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

    /// Fetches or sets a value associated with the context.
    fn value<V>(&mut self, key: &'static str) -> &mut V
    where
        V: Any + Default;

    /// Takes a value associated with the context.
    ///
    /// The previous value is removed so subsequent fetches will return the default value.
    fn take_value<V>(&mut self, key: &'static str) -> Box<V>
    where
        V: Any + Default;
}

/// Dispatch context for the whole batch.
pub struct DispatchContext<'a> {
    pub(crate) mode: Mode,

    pub(crate) runtime_header: &'a roothash::Header,
    pub(crate) runtime_round_results: &'a roothash::RoundResults,
    pub(crate) runtime_storage: storage::MKVSStore<&'a mut dyn mkvs::MKVS>,
    // TODO: linked consensus layer block
    pub(crate) consensus_state: &'a consensus::state::ConsensusState,
    pub(crate) io_ctx: Arc<IoContext>,
    pub(crate) logger: slog::Logger,

    /// The runtime's methods, in case you need to look them up for some reason.
    pub(crate) methods: &'a MethodRegistry,

    pub(crate) block_tags: Tags,

    /// Maximum number of messages that can be emitted.
    pub(crate) max_messages: u32,
    /// Emitted messages.
    pub(crate) messages: Vec<(roothash::Message, MessageEventHookInvocation)>,

    /// Per-context values.
    pub(crate) values: BTreeMap<&'static str, Box<dyn Any>>,
}

impl<'a> DispatchContext<'a> {
    /// Create a new dispatch context from the low-level runtime context.
    pub(crate) fn from_runtime(
        ctx: &'a RuntimeContext<'_>,
        mkvs: &'a mut dyn mkvs::MKVS,
        methods: &'a MethodRegistry,
    ) -> Self {
        let mode = if ctx.check_only {
            Mode::CheckTx
        } else {
            Mode::ExecuteTx
        };
        Self {
            mode,
            runtime_header: ctx.header,
            runtime_round_results: ctx.round_results,
            runtime_storage: storage::MKVSStore::new(ctx.io_ctx.clone(), mkvs),
            consensus_state: &ctx.consensus_state,
            io_ctx: ctx.io_ctx.clone(),
            logger: get_logger("runtime-sdk")
                .new(o!("ctx" => "dispatch", "mode" => Into::<&'static str>::into(&mode))),
            methods,
            block_tags: Tags::new(),
            max_messages: ctx.max_messages,
            messages: Vec::new(),
            values: BTreeMap::new(),
        }
    }

    /// Executes a function with the transaction-specific context set.
    pub fn with_tx<F, R>(&mut self, tx: transaction::Transaction, f: F) -> R
    where
        F: FnOnce(TxContext<'_, '_>, transaction::Call) -> R,
    {
        // Create a store wrapped by an overlay store so we can either rollback or commit.
        let store = storage::OverlayStore::new(&mut self.runtime_storage);

        let tx_ctx = TxContext {
            mode: self.mode,
            runtime_header: self.runtime_header,
            runtime_round_results: self.runtime_round_results,
            consensus_state: self.consensus_state,
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
        };
        f(tx_ctx, tx.call)
    }

    /// Run something with a simulation context based on this context.
    /// The simulation context collects its own messages and starts with an empty set of context
    /// values.
    /// Runtime storage is shared with this context, so don't go committing it.
    pub fn with_simulation<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut DispatchContext<'_>) -> R,
    {
        let mut sim_ctx = DispatchContext {
            mode: Mode::SimulateTx,
            runtime_header: self.runtime_header,
            runtime_round_results: self.runtime_round_results,
            runtime_storage: self.runtime_storage,
            io_ctx: self.io_ctx.clone(),
            methods: self.methods,
            max_messages: self.max_messages,
            messages: Vec::new(),
            values: BTreeMap::new(),
        };
        f(&mut sim_ctx)
    }
}

impl<'a> Context for DispatchContext<'a> {
    type S = storage::MKVSStore<&'a mut dyn mkvs::MKVS>;

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

    fn runtime_state(&mut self) -> &mut Self::S {
        &mut self.runtime_storage
    }

    fn consensus_state(&self) -> &consensus::state::ConsensusState {
        &self.consensus_state
    }

    fn tx_auth_info(&self) -> Option<&transaction::AuthInfo> {
        None
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

    fn value<V>(&mut self, key: &'static str) -> &mut V
    where
        V: Any + Default,
    {
        self.values
            .entry(key)
            .or_insert_with(|| Box::new(V::default()))
            .downcast_mut()
            .expect("type should stay the same")
    }

    fn take_value<V>(&mut self, key: &'static str) -> Box<V>
    where
        V: Any + Default,
    {
        self.values
            .remove(key)
            .map(|x| x.downcast().expect("type should stay the same"))
            .unwrap_or_default()
    }
}

/// Per-transaction/method dispatch sub-context.
pub struct TxContext<'a, 'b> {
    mode: Mode,

    runtime_header: &'a roothash::Header,
    runtime_round_results: &'a roothash::RoundResults,
    consensus_state: &'a consensus::state::ConsensusState,
    // TODO: linked consensus layer block
    store: storage::OverlayStore<&'b mut storage::MKVSStore<&'a mut dyn mkvs::MKVS>>,

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
    values: &'b mut BTreeMap<&'static str, Box<dyn Any>>,

    /// Per-transaction values.
    tx_values: BTreeMap<&'static str, Box<dyn Any>>,
}

impl<'a, 'b> Context for TxContext<'a, 'b> {
    type S = storage::OverlayStore<&'b mut storage::MKVSStore<&'a mut dyn mkvs::MKVS>>;

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

    fn runtime_state(&mut self) -> &mut Self::S {
        &mut self.store
    }

    fn consensus_state(&self) -> &consensus::state::ConsensusState {
        self.consensus_state
    }

    fn tx_auth_info(&self) -> Option<&transaction::AuthInfo> {
        Some(&self.tx_auth_info)
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

    fn value<V>(&mut self, key: &'static str) -> &mut V
    where
        V: Any + Default,
    {
        self.values
            .entry(key)
            .or_insert_with(|| Box::new(V::default()))
            .downcast_mut()
            .expect("type should stay the same")
    }

    fn take_value<V>(&mut self, key: &'static str) -> Box<V>
    where
        V: Any + Default,
    {
        self.values
            .remove(key)
            .map(|x| x.downcast().expect("type should stay the same"))
            .unwrap_or_default()
    }

    /// Fetches or sets a value associated with the transaction.
    pub fn tx_value<V>(&mut self, key: &'static str) -> &mut V
    where
        V: Any + Default,
    {
        self.tx_values
            .entry(key)
            .or_insert_with(|| Box::new(V::default()))
            .downcast_mut()
            .expect("type should stay the same")
    }

    /// Takes a value associated with the transaction.
    ///
    /// The previous value is removed so subsequent fetches will return the default value.
    pub fn take_tx_value<V>(&mut self, key: &'static str) -> Box<V>
    where
        V: Any + Default,
    {
        self.tx_values
            .remove(key)
            .map(|x| x.downcast().expect("type should stay the same"))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod test {
    use oasis_core_runtime::common::cbor;

    use super::*;
    use crate::testing::mock::Mock;

    #[test]
    fn test_value() {
        let mut mock = Mock::default();
        let mut ctx = mock.create_ctx();

        let x: &mut Option<u64> = ctx.value("module.TestKey");
        assert_eq!(x, &None, "default value should be created");
        *x = Some(42);

        let y: &mut Option<u64> = ctx.value("module.TestKey");
        assert_eq!(y, &Some(42));

        let z: Box<Option<u64>> = ctx.take_value("module.TestKey");
        assert_eq!(z, Box::new(Some(42)));

        let y: &mut Option<u64> = ctx.value("module.TestKey");
        assert_eq!(y, &None);
    }

    #[test]
    #[should_panic]
    fn test_value_type_change() {
        let mut mock = Mock::default();
        let mut ctx = mock.create_ctx();

        let x: &mut Option<u64> = ctx.value("module.TestKey");
        assert_eq!(x, &None, "default value should be created");
        *x = Some(42);

        // Changing the type of a key should result in a panic.
        ctx.value::<Option<u32>>("module.TestKey");
    }

    #[test]
    fn test_value_tx_context() {
        let mut mock = Mock::default();
        let mut ctx = mock.create_ctx();

        let x: &mut Option<u64> = ctx.value("module.TestKey");
        assert_eq!(x, &None, "default value should be created");
        *x = Some(42);

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
            let y: &mut Option<u64> = tx_ctx.value("module.TestKey");
            assert_eq!(y, &Some(42));

            *y = Some(48);

            let a: &mut Option<u64> = tx_ctx.tx_value("module.TestTxKey");
            assert_eq!(a, &None);

            *a = Some(65);

            let b: &mut Option<u64> = tx_ctx.tx_value("module.TestTxKey");
            assert_eq!(b, &Some(65));

            let c: &mut Option<u64> = tx_ctx.tx_value("module.TestTakeTxKey");
            *c = Some(67);
            let d: Box<Option<u64>> = tx_ctx.take_tx_value("module.TestTakeTxKey");
            assert_eq!(d, Box::new(Some(67)));
            let e: &mut Option<u64> = tx_ctx.tx_value("module.TestTakeTxKey");
            assert_eq!(e, &None);
        });

        let x: &mut Option<u64> = ctx.value("module.TestKey");
        assert_eq!(x, &Some(48));

        ctx.with_tx(tx, |mut tx_ctx, _call| {
            let z: Box<Option<u64>> = tx_ctx.take_value("module.TestKey");
            assert_eq!(z, Box::new(Some(48)));

            let a: &mut Option<u64> = tx_ctx.tx_value("module.TestTxKey");
            assert_eq!(a, &None);
        });

        let y: &mut Option<u64> = ctx.value("module.TestKey");
        assert_eq!(y, &None);
    }

    #[test]
    #[should_panic]
    fn test_value_tx_context_type_change() {
        let mut mock = Mock::default();
        let mut ctx = mock.create_ctx();

        let x: &mut Option<u64> = ctx.value("module.TestKey");
        assert_eq!(x, &None, "default value should be created");
        *x = Some(42);

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
            tx_ctx.value::<Option<u32>>("module.TestKey");
        });
    }
}
