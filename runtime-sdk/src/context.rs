//! Execution context.
use std::{any::Any, collections::BTreeMap, sync::Arc};

use io_context::Context as IoContext;
use thiserror::Error;

use oasis_core_runtime::{
    consensus::roothash,
    storage::mkvs,
    transaction::{context::Context as RuntimeContext, tags::Tags},
};

use crate::{
    event::Event,
    module::MethodRegistry,
    storage,
    types::{address::Address, transaction},
};

/// Context-related errors.
#[derive(Error, Debug)]
pub enum Error {
    #[error("too many emitted runtime messages")]
    TooManyMessages,
}

/// Transaction execution mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    ExecuteTx,
    CheckTx,
    SimulateTx,
}

/// Dispatch context for the whole batch.
pub struct DispatchContext<'a> {
    pub(crate) mode: Mode,

    pub(crate) runtime_header: &'a roothash::Header,
    pub(crate) runtime_round_results: &'a roothash::RoundResults,
    pub(crate) runtime_storage: &'a mut dyn mkvs::MKVS,
    // TODO: linked consensus layer block
    // TODO: linked consensus layer state storage (or just expose high-level stuff)
    pub(crate) io_ctx: Arc<IoContext>,

    /// The runtime's methods, in case you need to look them up for some reason.
    pub(crate) methods: &'a MethodRegistry,

    /// Maximum number of messages that can be emitted.
    pub(crate) max_messages: u32,
    /// Emitted messages.
    pub(crate) messages: Vec<roothash::Message>,

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
        Self {
            mode: if ctx.check_only {
                Mode::CheckTx
            } else {
                Mode::ExecuteTx
            },
            runtime_header: ctx.header,
            runtime_round_results: ctx.round_results,
            runtime_storage: mkvs,
            io_ctx: ctx.io_ctx.clone(),
            methods,
            max_messages: ctx.max_messages,
            messages: Vec::new(),
            values: BTreeMap::new(),
        }
    }

    /// Last runtime block header.
    pub fn runtime_header(&self) -> &roothash::Header {
        self.runtime_header
    }

    /// Results of executing the last successful runtime round.
    pub fn runtime_round_results(&self) -> &roothash::RoundResults {
        self.runtime_round_results
    }

    /// Runtime state store.
    pub fn runtime_state(&mut self) -> storage::MKVSStore<&mut dyn mkvs::MKVS> {
        storage::MKVSStore::new(self.io_ctx.clone(), &mut self.runtime_storage)
    }

    /// Emits runtime messages
    pub fn emit_messages(&mut self, mut msgs: Vec<roothash::Message>) -> Result<(), Error> {
        // Check against maximum number of messages that can be emitted per round.
        if self.messages.len() > self.max_messages as usize {
            return Err(Error::TooManyMessages);
        }

        self.messages.append(&mut msgs);
        Ok(())
    }

    /// Finalize the context and return the emitted runtime messages, consuming the context.
    pub fn commit(self) -> Vec<roothash::Message> {
        self.messages
    }

    /// Fetches or sets a value associated with the context.
    pub fn value<V>(&mut self, key: &'static str) -> &mut V
    where
        V: Any + Default,
    {
        self.values
            .entry(key)
            .or_insert_with(|| Box::new(V::default()))
            .downcast_mut()
            .expect("type should stay the same")
    }

    /// Takes a value associated with the context.
    ///
    /// The previous value is removed so subsequent fetches will return the default value.
    pub fn take_value<V>(&mut self, key: &'static str) -> Box<V>
    where
        V: Any + Default,
    {
        self.values
            .remove(key)
            .map(|x| x.downcast().expect("type should stay the same"))
            .unwrap_or_default()
    }

    /// Executes a function with the transaction-specific context set.
    pub fn with_tx<F, R>(&mut self, tx: transaction::Transaction, f: F) -> R
    where
        F: FnOnce(TxContext<'_, '_>, transaction::Call) -> R,
    {
        // Create a store wrapped by an overlay store so we can either rollback or commit.
        let store = storage::MKVSStore::new(self.io_ctx.clone(), &mut self.runtime_storage);
        let store = storage::OverlayStore::new(store);

        let tx_ctx = TxContext {
            mode: self.mode,
            runtime_header: self.runtime_header,
            runtime_round_results: self.runtime_round_results,
            store,
            tx_auth_info: tx.auth_info,
            tags: Tags::new(),
            // NOTE: Since a limit is enforced (which is a u32) this cast is always safe.
            message_offset: self.messages.len() as u32,
            max_messages: self.max_messages.saturating_sub(self.messages.len() as u32),
            messages: Vec::new(),
            values: &mut self.values,
            tx_values: BTreeMap::new(),
        };
        f(tx_ctx, tx.call)
    }
}

/// Per-transaction dispatch context.
pub struct TxContext<'a, 'b> {
    mode: Mode,

    runtime_header: &'a roothash::Header,
    runtime_round_results: &'a roothash::RoundResults,
    // TODO: linked consensus layer block
    // TODO: linked consensus layer state storage (or just expose high-level stuff)
    store: storage::OverlayStore<storage::MKVSStore<&'b mut &'a mut dyn mkvs::MKVS>>,

    /// Transaction authentication info.
    tx_auth_info: transaction::AuthInfo,

    /// Emitted tags.
    tags: Tags,

    /// Offset for emitted message indices (as those are global).
    message_offset: u32,
    /// Maximum number of messages that can be emitted.
    max_messages: u32,
    /// Emitted messages.
    messages: Vec<roothash::Message>,

    /// Per-context values.
    values: &'b mut BTreeMap<&'static str, Box<dyn Any>>,

    /// Per-transaction values.
    tx_values: BTreeMap<&'static str, Box<dyn Any>>,
}

impl<'a, 'b> TxContext<'a, 'b> {
    /// Whether the transaction is just being checked for validity.
    pub fn is_check_only(&self) -> bool {
        self.mode == Mode::CheckTx
    }

    /// Whether the transaction is just being simulated.
    pub fn is_simulation(&self) -> bool {
        self.mode == Mode::SimulateTx
    }

    /// Last runtime block header.
    pub fn runtime_header(&self) -> &roothash::Header {
        self.runtime_header
    }

    /// Results of executing the last successful runtime round.
    pub fn runtime_round_results(&self) -> &roothash::RoundResults {
        self.runtime_round_results
    }

    /// Runtime state store.
    pub fn runtime_state(
        &mut self,
    ) -> &mut storage::OverlayStore<storage::MKVSStore<&'b mut &'a mut dyn mkvs::MKVS>> {
        &mut self.store
    }

    /// Transaction authentication information.
    pub fn tx_auth_info(&self) -> &transaction::AuthInfo {
        &self.tx_auth_info
    }

    /// Authenticated address of the caller.
    ///
    /// In case there are multiple signers of a transaction, this will return the address
    /// corresponding to the first signer.
    pub fn tx_caller_address(&self) -> Address {
        Address::from_pk(&self.tx_auth_info().signer_info[0].public_key)
    }

    /// Emits an event.
    pub fn emit_event<E: Event>(&mut self, event: E) {
        self.tags.push(event.to_tag());
    }

    /// Attempts to emit a runtime message.
    ///
    /// Returns an index of the emitted message that can be used to correlate the corresponding
    /// result after the message has been processed (in the next round).
    pub fn emit_message(&mut self, msg: roothash::Message) -> Result<u32, Error> {
        // Check against maximum number of messages that can be emitted per round.
        if self.messages.len() >= self.max_messages as usize {
            return Err(Error::TooManyMessages);
        }

        self.messages.push(msg);
        // NOTE: The cast to u32 is safe as the maximum is u32 so the length is representable.
        Ok(self.message_offset + (self.messages.len() as u32) - 1)
    }

    /// Commit any changes made to storage, return any emitted tags and runtime messages. It
    /// consumes the transaction context.
    pub fn commit(self) -> (Tags, Vec<roothash::Message>) {
        self.store.commit();
        (self.tags, self.messages)
    }

    /// Fetches or sets a value associated with the context.
    pub fn value<V>(&mut self, key: &'static str) -> &mut V
    where
        V: Any + Default,
    {
        self.values
            .entry(key)
            .or_insert_with(|| Box::new(V::default()))
            .downcast_mut()
            .expect("type should stay the same")
    }

    /// Takes a value associated with the context.
    ///
    /// The previous value is removed so subsequent fetches will return the default value.
    pub fn take_value<V>(&mut self, key: &'static str) -> Box<V>
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
