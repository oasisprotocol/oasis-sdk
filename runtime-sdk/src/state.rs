use std::{
    any::Any,
    cell::RefCell,
    collections::btree_map::{BTreeMap, Entry},
    fmt,
    marker::PhantomData,
    mem,
};

use oasis_core_runtime::{common::crypto::hash::Hash, consensus::roothash, storage::mkvs};

use crate::{
    context::Context,
    crypto::{random::RootRng, signature::PublicKey},
    event::{Event, EventTag, EventTags},
    modules::core::Error,
    storage::{MKVSStore, NestedStore, OverlayStore, Store},
    types::{address::Address, message::MessageEventHookInvocation, transaction},
};

/// Execution mode.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Mode {
    /// Actually execute transactions during block production.
    #[default]
    Execute,
    /// Check that transactions are valid for local acceptance into the transaction pool.
    Check,
    /// Simulate transaction outcomes (e.g. for gas estimation).
    Simulate,
    /// Check that transactions are still valid before scheduling.
    PreSchedule,
}

const MODE_CHECK: &str = "check";
const MODE_EXECUTE: &str = "execute";
const MODE_SIMULATE: &str = "simulate";
const MODE_PRE_SCHEDULE: &str = "pre_schedule";

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.into())
    }
}

impl From<&Mode> for &'static str {
    fn from(m: &Mode) -> Self {
        match m {
            Mode::Check => MODE_CHECK,
            Mode::Execute => MODE_EXECUTE,
            Mode::Simulate => MODE_SIMULATE,
            Mode::PreSchedule => MODE_PRE_SCHEDULE,
        }
    }
}

/// Information about the execution environment.
#[derive(Clone, Default, Debug)]
pub struct Environment {
    mode: Mode,
    tx: Option<TransactionWithMeta>,
    internal: bool,
}

impl Environment {
    /// Execution mode.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Whether the execution mode is such that only checks should be performed.
    pub fn is_check_only(&self) -> bool {
        matches!(self.mode, Mode::Check | Mode::PreSchedule)
    }

    /// Whether the execution mode is `Mode::PreSchedule`.
    pub fn is_pre_schedule(&self) -> bool {
        matches!(self.mode, Mode::PreSchedule)
    }

    /// Whether the execution mode is `Mode::Simulate`.
    pub fn is_simulation(&self) -> bool {
        matches!(self.mode, Mode::Simulate)
    }

    /// Whether the execution mode is `Mode::Execute`.
    pub fn is_execute(&self) -> bool {
        matches!(self.mode, Mode::Execute)
    }

    /// Whether there is an active transaction in the current environment.
    pub fn is_transaction(&self) -> bool {
        self.tx.is_some()
    }

    /// An active transaction's index (order) within the block.
    ///
    /// # Panics
    ///
    /// This method will panic if called outside a transaction environment.
    pub fn tx_index(&self) -> usize {
        self.tx
            .as_ref()
            .map(|tx| tx.index)
            .expect("only in transaction environment")
    }

    /// An active transaction's size in bytes.
    ///
    /// # Panics
    ///
    /// This method will panic if called outside a transaction environment.
    pub fn tx_size(&self) -> u32 {
        self.tx
            .as_ref()
            .map(|tx| tx.size)
            .expect("only in transaction environment")
    }

    /// An active transaction's authentication information.
    ///
    /// # Panics
    ///
    /// This method will panic if called outside a transaction environment.
    pub fn tx_auth_info(&self) -> &transaction::AuthInfo {
        self.tx
            .as_ref()
            .map(|tx| &tx.data.auth_info)
            .expect("only in transaction environment")
    }

    /// An active transaction's call format.
    ///
    /// # Panics
    ///
    /// This method will panic if called outside a transaction environment.
    pub fn tx_call_format(&self) -> transaction::CallFormat {
        self.tx
            .as_ref()
            .map(|tx| tx.data.call.format)
            .expect("only in transaction environment")
    }

    /// An active transaction's read only flag.
    ///
    /// # Panics
    ///
    /// This method will panic if called outside a transaction environment.
    pub fn is_read_only(&self) -> bool {
        self.tx
            .as_ref()
            .map(|tx| tx.data.call.read_only)
            .expect("only in transaction environment")
    }

    /// Whether the current execution environment is part of an internal subcall.
    pub fn is_internal(&self) -> bool {
        self.internal
    }

    /// Authenticated address of the caller.
    ///
    /// In case there are multiple signers of a transaction, this will return the address
    /// corresponding to the first signer. If there are no signers, it returns the default address.
    ///
    /// # Panics
    ///
    /// This method will panic if called outside a transaction environment.
    pub fn tx_caller_address(&self) -> Address {
        self.tx_auth_info()
            .signer_info
            .first()
            .map(|si| si.address_spec.address())
            .unwrap_or_default()
    }

    /// Authenticated caller public key if available.
    ///
    /// In case there are multiple signers of a transaction, this will return the public key
    /// corresponding to the first signer. If there are no signers or if the address specification
    /// does not represent a single public key, it returns `None`.
    ///
    /// # Panics
    ///
    /// This method will panic if called outside a transaction environment.
    pub fn tx_caller_public_key(&self) -> Option<PublicKey> {
        self.tx_auth_info()
            .signer_info
            .first()
            .and_then(|si| si.address_spec.public_key())
    }
}

/// Decoded transaction with additional metadata.
#[derive(Clone, Debug)]
pub struct TransactionWithMeta {
    /// Decoded transaction.
    pub data: transaction::Transaction,
    /// Transaction size.
    pub size: u32,
    /// Transaction index within the batch.
    pub index: usize,
    /// Transaction hash.
    pub hash: Hash,
}

impl TransactionWithMeta {
    /// Create transaction with metadata for an internally generated transaction.
    ///
    /// Internally generated transactions have zero size, index and hash.
    pub fn internal(tx: transaction::Transaction) -> Self {
        Self {
            data: tx,
            size: 0,
            index: 0,
            hash: Default::default(),
        }
    }
}

#[cfg(any(test, feature = "test"))]
impl From<transaction::Transaction> for TransactionWithMeta {
    fn from(tx: transaction::Transaction) -> Self {
        Self::internal(tx) // For use in tests.
    }
}

/// Environment modification options.
#[derive(Clone, Default, Debug)]
pub struct Options {
    pub mode: Option<Mode>,
    pub tx: Option<TransactionWithMeta>,
    pub internal: Option<bool>,
    pub rng_local_entropy: bool,
}

impl Options {
    /// Create options with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Change the execution mode of the environment.
    pub fn with_mode(self, mode: Mode) -> Self {
        Self {
            mode: Some(mode),
            ..self
        }
    }

    /// Change the active transaction of the environment.
    pub fn with_tx(self, tx: TransactionWithMeta) -> Self {
        Self {
            tx: Some(tx),
            ..self
        }
    }

    /// Change the internal flag of the environment.
    pub fn with_internal(self, internal: bool) -> Self {
        Self {
            internal: Some(internal),
            ..self
        }
    }

    /// Request for local entropy to be mixed into the current RNG.
    ///
    /// # Determinisim
    ///
    /// Using this method will result in non-deterministic behavior as the node's local entropy is
    /// mixed into the RNG. As such, this method should only be used in cases where non-determinism
    /// is not problematic (e.g. local queries).
    pub fn with_rng_local_entropy(self) -> Self {
        Self {
            rng_local_entropy: true,
            ..self
        }
    }
}

/// Mutable block state of a runtime.
///
/// The state includes storage, emitted events, messages to consensus layer, etc. States can be
/// nested via `open`, `commit` and `rollback` methods which behave like transactions.
pub struct State {
    parent: Option<Box<State>>,
    store: Option<OverlayStore<Box<dyn Store>>>,

    events: EventTags,
    unconditional_events: EventTags,
    messages: Vec<(roothash::Message, MessageEventHookInvocation)>,

    block_values: BTreeMap<&'static str, Box<dyn Any>>,
    hidden_block_values: Option<BTreeMap<&'static str, Box<dyn Any>>>,
    local_values: BTreeMap<&'static str, Box<dyn Any>>,

    rng: Option<RootRng>,
    hidden_rng: Option<RootRng>,
    env: Environment,

    always_rollback: bool,
}

impl State {
    /// Initialize the state with the given options.
    fn init(&mut self, opts: Options) {
        if let Some(mode) = opts.mode {
            // Change mode.
            self.env.mode = mode;
            // If we have enabled pre-schedule or simulation mode, always rollback state and hide
            // block values to prevent leaking them.
            if matches!(mode, Mode::PreSchedule | Mode::Simulate) {
                self.always_rollback = true;
                self.hide_block_values();
            }
        }

        if let Some(tx) = opts.tx {
            // Change RNG state.
            self.rng.as_mut().unwrap().append_tx(tx.hash);
            // Change tx metadata.
            self.env.tx = Some(tx);
        }

        if let Some(internal) = opts.internal {
            self.env.internal = internal;
            if internal {
                self.hide_block_values();
            }
        }

        if opts.rng_local_entropy {
            // Append local entropy to RNG state.
            self.rng.as_mut().unwrap().append_local_entropy();
        }

        if !matches!(self.env.mode, Mode::PreSchedule) {
            // Record opening a child state in the RNG.
            self.rng.as_mut().unwrap().append_subcontext();
        } else {
            // Use an invalid RNG as its use is not allowed in pre-schedule context.
            self.disable_rng();
        }
    }

    /// Open a child state after which self will point to the child state.
    pub fn open(&mut self) {
        let mut parent = Self {
            parent: None,
            store: None,
            events: EventTags::new(),
            unconditional_events: EventTags::new(),
            messages: Vec::new(),
            block_values: BTreeMap::new(),
            hidden_block_values: None,
            local_values: BTreeMap::new(),
            rng: None,
            hidden_rng: None,
            env: self.env.clone(),
            always_rollback: false,
        };
        mem::swap(&mut parent, self);

        // Wrap parent store to create an overlay child store.
        self.store = parent
            .store
            .take()
            .map(|pstore| OverlayStore::new(Box::new(pstore) as Box<dyn Store>));

        // Take block values map. We will put it back after commit/rollback.
        mem::swap(&mut parent.block_values, &mut self.block_values);
        // Take RNG. We will put it back after commit/rollback.
        mem::swap(&mut parent.rng, &mut self.rng);

        self.parent = Some(Box::new(parent));
    }

    fn convert_store(store: Box<dyn Store>) -> OverlayStore<Box<dyn Store>> {
        let raw = Box::into_raw(store);
        unsafe {
            // SAFETY: This is safe because we always wrap child stores into OverlayStore.
            *Box::from_raw(raw as *mut OverlayStore<Box<dyn Store>>)
        }
    }

    /// Commit the current state and return to its parent state.
    ///
    /// # Panics
    ///
    /// This method will panic when attempting to commit the root state.
    pub fn commit(&mut self) {
        if self.always_rollback {
            self.rollback();
        } else {
            self._commit();
        }
    }

    fn _commit(&mut self) {
        let mut child = *self.parent.take().expect("cannot commit on root state");
        mem::swap(&mut child, self);

        // Commit storage.
        self.store = child
            .store
            .take()
            .map(|cstore| Self::convert_store(cstore.commit()));

        // Propagate messages.
        self.messages.extend(child.messages);

        // Propagate events.
        for (key, event) in child.events {
            let events = self.events.entry(key).or_default();
            events.extend(event);
        }
        for (key, event) in child.unconditional_events {
            let events = self.unconditional_events.entry(key).or_default();
            events.extend(event);
        }

        // Put back per-block values.
        if let Some(mut block_values) = child.hidden_block_values {
            mem::swap(&mut block_values, &mut self.block_values); // Block values were hidden.
        } else {
            mem::swap(&mut child.block_values, &mut self.block_values);
        }
        // Always drop local values.

        // Put back RNG.
        if child.hidden_rng.is_some() {
            mem::swap(&mut child.hidden_rng, &mut self.rng); // RNG was hidden.
        } else {
            mem::swap(&mut child.rng, &mut self.rng);
        }
    }

    /// Rollback the current state and return to its parent state.
    ///
    /// # Panics
    ///
    /// This method will panic when attempting to rollback the root state.
    pub fn rollback(&mut self) {
        let mut child = *self.parent.take().expect("cannot rollback on root state");
        mem::swap(&mut child, self);

        // Rollback storage.
        self.store = child
            .store
            .take()
            .map(|cstore| Self::convert_store(cstore.rollback()));

        // Always put back per-block values.
        if let Some(mut block_values) = child.hidden_block_values {
            mem::swap(&mut block_values, &mut self.block_values); // Block values were hidden.
        } else {
            mem::swap(&mut child.block_values, &mut self.block_values);
        }
        // Always drop local values.

        // Always put back RNG.
        if child.hidden_rng.is_some() {
            mem::swap(&mut child.hidden_rng, &mut self.rng); // RNG was hidden.
        } else {
            mem::swap(&mut child.rng, &mut self.rng);
        }
    }

    /// Fetches a block state value entry.
    ///
    /// Block values live as long as the root `State` and are propagated to child states. They are
    /// not affected by state rollbacks. If you need state-scoped values, use local values.
    pub fn block_value<V: Any>(&mut self, key: &'static str) -> StateValue<'_, V> {
        StateValue::new(self.block_values.entry(key))
    }

    /// Fetches a local state value entry.
    ///
    /// Local values only live as long as the current `State`, are dropped upon exiting to parent
    /// state and child states start with an empty set. If you need longer-lived values, use block
    /// values.
    pub fn local_value<V: Any>(&mut self, key: &'static str) -> StateValue<'_, V> {
        StateValue::new(self.local_values.entry(key))
    }

    /// Hides block values from the current state which will have an empty set of values after this
    /// method returns. Hidden values will be restored upon exit to parent state.
    pub fn hide_block_values(&mut self) {
        if self.parent.is_none() {
            // Allowing hiding on root state would prevent those values from ever being recovered.
            panic!("cannot hide block values on root state");
        }
        if self.hidden_block_values.is_some() {
            return; // Parent block values already hidden.
        }

        self.hidden_block_values = Some(mem::take(&mut self.block_values));
    }

    /// Emitted messages count returns the number of messages emitted so far across this and all
    /// parent states.
    pub fn emitted_messages_count(&self) -> usize {
        self.messages.len()
            + self
                .parent
                .as_ref()
                .map(|p| p.emitted_messages_count())
                .unwrap_or_default()
    }

    /// Emitted messages count returns the number of messages emitted so far in this state, not
    /// counting any parent states.
    pub fn emitted_messages_local_count(&self) -> usize {
        self.messages.len()
    }

    /// Maximum number of messages that can be emitted.
    pub fn emitted_messages_max<C: Context>(&self, ctx: &C) -> u32 {
        if self.env.is_transaction() {
            let limit = self.env.tx_auth_info().fee.consensus_messages;
            if limit > 0 {
                limit
            } else {
                ctx.max_messages() // Zero means an implicit limit by gas use.
            }
        } else {
            ctx.max_messages()
        }
    }

    /// Queue a message to be emitted by the runtime for consensus layer to process.
    pub fn emit_message<C: Context>(
        &mut self,
        ctx: &C,
        msg: roothash::Message,
        hook: MessageEventHookInvocation,
    ) -> Result<(), Error> {
        // Check against maximum number of messages that can be emitted per round.
        if self.emitted_messages_count() >= self.emitted_messages_max(ctx) as usize {
            return Err(Error::OutOfMessageSlots);
        }

        self.messages.push((msg, hook));

        Ok(())
    }

    /// Take all messages accumulated in the current state.
    pub fn take_messages(&mut self) -> Vec<(roothash::Message, MessageEventHookInvocation)> {
        mem::take(&mut self.messages)
    }

    /// Emit an event.
    pub fn emit_event<E: Event>(&mut self, event: E) {
        self.emit_event_raw(event.into_event_tag());
    }

    /// Emit a raw event.
    pub fn emit_event_raw(&mut self, etag: EventTag) {
        let events = self.events.entry(etag.key).or_default();
        events.push(etag.value);
    }

    /// Emit an unconditional event.
    ///
    /// The only difference to regular events is that these are handled as a separate set.
    pub fn emit_unconditional_event<E: Event>(&mut self, event: E) {
        let etag = event.into_event_tag();
        let events = self.unconditional_events.entry(etag.key).or_default();
        events.push(etag.value);
    }

    /// Take all regular events accumulated in the current state.
    pub fn take_events(&mut self) -> EventTags {
        mem::take(&mut self.events)
    }

    /// Take all unconditional events accumulated in the current state.
    pub fn take_unconditional_events(&mut self) -> EventTags {
        mem::take(&mut self.unconditional_events)
    }

    /// Take all events accumulated in the current state and return the merged set.
    pub fn take_all_events(&mut self) -> EventTags {
        let mut events = self.take_events();
        let unconditional_events = self.take_unconditional_events();

        for (key, val) in unconditional_events {
            let tag = events.entry(key).or_default();
            tag.extend(val)
        }

        events
    }

    /// Store associated with the state.
    ///
    /// # Panics
    ///
    /// This method will panic if no store exists.
    pub fn store(&mut self) -> &mut dyn Store {
        self.store.as_mut().unwrap()
    }

    /// Whether the store associated with the state has any pending updates.
    pub fn has_pending_store_updates(&self) -> bool {
        self.store
            .as_ref()
            .map(|store| store.has_pending_updates())
            .unwrap_or_default()
    }

    /// Size (in bytes) of any pending updates in the associated store.
    pub fn pending_store_update_byte_size(&self) -> usize {
        self.store
            .as_ref()
            .map(|store| store.pending_update_byte_size())
            .unwrap_or_default()
    }

    /// Random number generator.
    pub fn rng(&mut self) -> &mut RootRng {
        self.rng.as_mut().unwrap()
    }

    /// Disables the RNG by replacing the instance with an invalid RNG.
    fn disable_rng(&mut self) {
        if self.parent.is_none() {
            // Allowing hiding on root state would prevent the RNG from ever being recovered.
            panic!("cannot hide the RNG on root state");
        }
        if self.hidden_rng.is_some() {
            return; // Parent RNG already hidden.
        }

        self.hidden_rng = mem::replace(&mut self.rng, Some(RootRng::invalid()));
    }

    /// Environment information.
    pub fn env(&self) -> &Environment {
        &self.env
    }

    /// Origin environment information.
    ///
    /// The origin environment is the first non-internal environment in the hierarchy.
    pub fn env_origin(&self) -> &Environment {
        match self.parent {
            Some(ref parent) if self.env.internal => parent.env_origin(),
            _ => &self.env,
        }
    }

    /// Returns the nesting level of the current state.
    pub fn level(&self) -> usize {
        if let Some(ref parent) = self.parent {
            parent.level() + 1
        } else {
            0
        }
    }
}

thread_local! {
    static CURRENT: RefCell<Vec<State>> = const { RefCell::new(Vec::new()) };
}

struct CurrentStateGuard;

impl Drop for CurrentStateGuard {
    fn drop(&mut self) {
        CURRENT.with(|c| {
            let root = c.borrow_mut().pop().expect("must have current state");
            // Commit root state as it has been wrapped in an overlay.
            let store = root
                .store
                .expect("must not have open child states after exiting root state");
            store.commit();
        });
    }
}

struct TransactionGuard(usize);

impl Drop for TransactionGuard {
    fn drop(&mut self) {
        let level = CurrentState::with(|state| state.level());

        // If transaction hasn't been either committed or reverted, rollback.
        if level == self.0 {
            CurrentState::rollback_transaction();
        }
    }
}

/// Result of a transaction helper closure.
pub enum TransactionResult<T> {
    Commit(T),
    Rollback(T),
}

impl From<()> for TransactionResult<()> {
    fn from(_: ()) -> TransactionResult<()> {
        TransactionResult::Commit(())
    }
}

impl<R, E> From<Result<R, E>> for TransactionResult<Result<R, E>> {
    fn from(v: Result<R, E>) -> TransactionResult<Result<R, E>> {
        match v {
            Ok(_) => TransactionResult::Commit(v),
            Err(_) => TransactionResult::Rollback(v),
        }
    }
}

/// State attached to the current thread.
pub struct CurrentState;

impl CurrentState {
    /// Attach a new state to the current thread and enter the state's context.
    ///
    /// The passed store is used as the root store.
    ///
    /// # Panics
    ///
    /// This method will panic if called from within a `CurrentState::with` block.
    pub fn enter<S, F, R>(root: S, f: F) -> R
    where
        S: Store,
        F: FnOnce() -> R,
    {
        Self::enter_opts(
            Options {
                mode: Some(Default::default()), // Make sure there is a default mode.
                ..Options::default()
            },
            root,
            f,
        )
    }

    /// Attach a new state to the current thread and enter the state's context.
    ///
    /// The passed store is used as the root store.
    ///
    /// # Panics
    ///
    /// This method will panic if called from within a `CurrentState::with` block or if the mode
    /// has not been explicitly set in `opts`.
    pub fn enter_opts<S, F, R>(opts: Options, mut root: S, f: F) -> R
    where
        S: Store,
        F: FnOnce() -> R,
    {
        let root = unsafe {
            // SAFETY: Keeping the root store is safe as it can only be accessed from the current
            // thread while we are running inside `CurrentState::enter` where we are holding a
            // mutable reference on it.
            std::mem::transmute::<&mut dyn Store, &mut (dyn Store + 'static)>(
                &mut root as &mut dyn Store,
            )
        };
        // Initialize the root state.
        let mode = opts
            .mode
            .expect("mode must be explicitly set on root state");
        let mut root = State {
            parent: None,
            store: Some(OverlayStore::new(Box::new(root) as Box<dyn Store>)),
            events: EventTags::new(),
            unconditional_events: EventTags::new(),
            messages: Vec::new(),
            block_values: BTreeMap::new(),
            hidden_block_values: None,
            local_values: BTreeMap::new(),
            rng: Some(RootRng::new(mode)),
            hidden_rng: None,
            env: Default::default(),
            always_rollback: false,
        };
        // Apply options to allow customization of the root state.
        root.init(opts);

        CURRENT.with(|c| {
            c.try_borrow_mut()
                .expect("must not re-enter from with block")
                .push(root)
        });
        let _guard = CurrentStateGuard; // Ensure current state is popped once we return.

        f()
    }

    /// Create an empty baseline state for the current thread.
    ///
    /// This should only be used in tests to have state always available.
    ///
    /// # Panics
    ///
    /// This method will panic if any states have been attached to the local thread or if called
    /// within a `CurrentState::with` block.
    #[doc(hidden)]
    pub(crate) fn init_local_fallback() {
        thread_local! {
            static BASE_STATE_INIT: RefCell<bool> = const { RefCell::new(false) };
        }

        BASE_STATE_INIT.with(|initialized| {
            // Initialize once per thread.
            if *initialized.borrow() {
                return;
            }
            *initialized.borrow_mut() = true;

            let root = mkvs::OverlayTree::new(
                mkvs::Tree::builder()
                    .with_root_type(mkvs::RootType::State)
                    .build(Box::new(mkvs::sync::NoopReadSyncer)),
            );
            let root = MKVSStore::new(root);

            // Initialize the root state.
            let root = State {
                parent: None,
                store: Some(OverlayStore::new(Box::new(root) as Box<dyn Store>)),
                events: EventTags::new(),
                unconditional_events: EventTags::new(),
                messages: Vec::new(),
                block_values: BTreeMap::new(),
                hidden_block_values: None,
                local_values: BTreeMap::new(),
                rng: Some(RootRng::new(Default::default())),
                hidden_rng: None,
                env: Default::default(),
                always_rollback: false,
            };

            CURRENT.with(|c| {
                let mut current = c
                    .try_borrow_mut()
                    .expect("must not re-enter from with block");
                assert!(
                    current.is_empty(),
                    "must have no prior states attached to local thread"
                );

                current.push(root);
            });
        });
    }

    /// Run a closure with the currently active state.
    ///
    /// # Panics
    ///
    /// This method will panic if called outside `CurrentState::enter` or if any transaction methods
    /// are called from the closure.
    pub fn with<F, R>(f: F) -> R
    where
        F: FnOnce(&mut State) -> R,
    {
        CURRENT.with(|c| {
            let mut current_ref = c.try_borrow_mut().expect("must not re-enter with");
            let current = current_ref.last_mut().expect("must enter context");

            f(current)
        })
    }

    /// Run a closure with the store of the currently active state.
    ///
    /// # Panics
    ///
    /// This method will panic if called outside `CurrentState::enter` or if any transaction methods
    /// are called from the closure.
    pub fn with_store<F, R>(f: F) -> R
    where
        F: FnOnce(&mut dyn Store) -> R,
    {
        Self::with(|state| f(state.store()))
    }

    /// Run a closure with the environment of the currently active state.
    ///
    /// # Panics
    ///
    /// This method will panic if called outside `CurrentState::enter` or if any transaction methods
    /// are called from the closure.
    pub fn with_env<F, R>(f: F) -> R
    where
        F: FnOnce(&Environment) -> R,
    {
        Self::with(|state| f(state.env()))
    }

    /// Run a closure with the origin environment of the currently active state.
    ///
    /// # Panics
    ///
    /// This method will panic if called outside `CurrentState::enter` or if any transaction methods
    /// are called from the closure.
    pub fn with_env_origin<F, R>(f: F) -> R
    where
        F: FnOnce(&Environment) -> R,
    {
        Self::with(|state| f(state.env_origin()))
    }

    /// Start a new transaction by opening a new child state.
    ///
    /// # Panics
    ///
    /// This method will panic if called outside `CurrentState::enter` or if called within a
    /// `CurrentState::with` block.
    pub fn start_transaction() {
        Self::with(|state| state.open());
    }

    /// Commit a previously started transaction.
    ///
    /// # Panics
    ///
    /// This method will panic if called outside `CurrentState::enter`, if there is no currently
    /// open transaction (started via `CurrentState::start_transaction`) or if called within a
    /// `CurrentState::with` block.
    pub fn commit_transaction() {
        Self::with(|state| state.commit());
    }

    /// Rollback a previously started transaction.
    ///
    /// # Panics
    ///
    /// This method will panic if called outside `CurrentState::enter`, if there is no currently
    /// open transaction (started via `CurrentState::start_transaction`) or if called within a
    /// `CurrentState::with` block.
    pub fn rollback_transaction() {
        Self::with(|state| state.rollback());
    }

    /// Run a closure within a state transaction.
    ///
    /// If the closure returns `TransactionResult::Commit(R)` then the child state is committed,
    /// otherwise the child state is rolled back.
    pub fn with_transaction<F, R, Rs>(f: F) -> R
    where
        F: FnOnce() -> Rs,
        Rs: Into<TransactionResult<R>>,
    {
        Self::with_transaction_opts(Options::default(), f)
    }

    /// Run a closure within a state transaction, allowing the caller to customize state.
    ///
    /// If the closure returns `TransactionResult::Commit(R)` then the child state is committed,
    /// otherwise the child state is rolled back.
    pub fn with_transaction_opts<F, R, Rs>(opts: Options, f: F) -> R
    where
        F: FnOnce() -> Rs,
        Rs: Into<TransactionResult<R>>,
    {
        let level = Self::with(|state| {
            state.open();
            state.init(opts);
            state.level()
        });
        let _guard = TransactionGuard(level); // Ensure transaction is always closed.

        match f().into() {
            TransactionResult::Commit(result) => {
                Self::commit_transaction();
                result
            }
            TransactionResult::Rollback(result) => {
                Self::rollback_transaction();
                result
            }
        }
    }
}

/// A per-state arbitrary value.
pub struct StateValue<'a, V> {
    inner: Entry<'a, &'static str, Box<dyn Any>>,
    _value: PhantomData<V>,
}

impl<'a, V: Any> StateValue<'a, V> {
    fn new(inner: Entry<'a, &'static str, Box<dyn Any>>) -> Self {
        Self {
            inner,
            _value: PhantomData,
        }
    }

    /// Gets a reference to the specified per-state value.
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

    /// Gets a mutable reference to the specified per-state value.
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

impl<'a, V: Any + Default> StateValue<'a, V> {
    /// Retrieves the existing value or inserts and returns the default.
    ///
    /// # Panics
    ///
    /// Panics if the retrieved type is not the type that was stored.
    pub fn or_default(self) -> &'a mut V {
        match self.inner {
            Entry::Occupied(oe) => oe.into_mut(),
            Entry::Vacant(ve) => ve.insert(Box::<V>::default()),
        }
        .downcast_mut()
        .expect("type should stay the same")
    }
}

#[cfg(test)]
mod test {
    use oasis_core_runtime::{
        common::versioned::Versioned,
        consensus::{roothash, staking},
        storage::mkvs,
    };

    use super::{CurrentState, Mode, Options, TransactionResult, TransactionWithMeta};
    use crate::{
        modules::core::Event,
        storage::{MKVSStore, Store},
        testing::mock::{self, Mock},
        types::message::MessageEventHookInvocation,
    };

    #[test]
    fn test_value() {
        CurrentState::init_local_fallback();

        CurrentState::with(|state| {
            let x = state.block_value::<u64>("module.TestKey").get();
            assert_eq!(x, None);

            state.block_value::<u64>("module.TestKey").set(42);

            let y = state.block_value::<u64>("module.TestKey").get();
            assert_eq!(y, Some(&42u64));

            let z = state.block_value::<u64>("module.TestKey").take();
            assert_eq!(z, Some(42u64));

            let y = state.block_value::<u64>("module.TestKey").get();
            assert_eq!(y, None);
        });
    }

    #[test]
    #[should_panic]
    fn test_value_type_change_block_value() {
        CurrentState::init_local_fallback();

        CurrentState::with(|state| {
            state.block_value::<u64>("module.TestKey").or_default();
            state.block_value::<u32>("module.TestKey").get();
        });
    }

    #[test]
    #[should_panic]
    fn test_value_type_change_local_value() {
        CurrentState::init_local_fallback();

        CurrentState::with(|state| {
            state.local_value::<u64>("module.TestKey").or_default();
            state.local_value::<u32>("module.TestKey").get();
        });
    }

    #[test]
    fn test_value_hidden_block_values() {
        CurrentState::init_local_fallback();

        CurrentState::with(|state| {
            state.block_value("module.TestKey").set(42u64);

            state.open();
            state.hide_block_values();

            let v = state.block_value::<u64>("module.TestKey").get();
            assert!(v.is_none(), "block values should not propagate when hidden");

            state.block_value("module.TestKey").set(48u64);

            state.commit();

            let v = state.block_value::<u64>("module.TestKey").get();
            assert_eq!(
                v,
                Some(&42u64),
                "block values should not propagate when hidden"
            );
        });
    }

    #[test]
    fn test_value_local() {
        CurrentState::init_local_fallback();

        CurrentState::with(|state| {
            state.block_value("module.TestKey").set(42u64);

            state.open();

            let mut y = state.block_value::<u64>("module.TestKey");
            let y = y.get_mut().unwrap();
            assert_eq!(*y, 42);
            *y = 48;

            let a = state.local_value::<u64>("module.TestTxKey").get();
            assert_eq!(a, None);
            state.local_value::<u64>("module.TestTxKey").set(65);

            let b = state.local_value::<u64>("module.TestTxKey").get();
            assert_eq!(b, Some(&65));

            let c = state
                .local_value::<u64>("module.TestTakeTxKey")
                .or_default();
            *c = 67;
            let d = state.local_value::<u64>("module.TestTakeTxKey").take();
            assert_eq!(d, Some(67));
            let e = state.local_value::<u64>("module.TestTakeTxKey").get();
            assert_eq!(e, None);

            state.rollback(); // Block values are always propagated.

            let x = state.block_value::<u64>("module.TestKey").get();
            assert_eq!(x, Some(&48));

            state.open();

            let z = state.block_value::<u64>("module.TestKey").take();
            assert_eq!(z, Some(48));

            let a = state.local_value::<u64>("module.TestTxKey").get();
            assert_eq!(a, None, "local values should not be propagated");

            state.rollback(); // Block values are always propagated.

            let y = state.block_value::<u64>("module.TestKey").get();
            assert_eq!(y, None);
        });
    }

    #[test]
    fn test_emit_messages() {
        let mut mock = Mock::default(); // Also creates local fallback state.
        let max_messages = mock.max_messages as usize;
        let ctx = mock.create_ctx();

        CurrentState::with(|state| {
            state.open();

            assert_eq!(state.emitted_messages_count(), 0);
            assert_eq!(state.emitted_messages_local_count(), 0);

            state
                .emit_message(
                    &ctx,
                    roothash::Message::Staking(Versioned::new(
                        0,
                        roothash::StakingMessage::Transfer(staking::Transfer::default()),
                    )),
                    MessageEventHookInvocation::new("test".to_string(), ""),
                )
                .expect("message emission should succeed");
            assert_eq!(state.emitted_messages_count(), 1);
            assert_eq!(state.emitted_messages_local_count(), 1);
            assert_eq!(state.emitted_messages_max(&ctx), max_messages as u32);

            state.open(); // Start child state.

            assert_eq!(state.emitted_messages_local_count(), 0);

            state
                .emit_message(
                    &ctx,
                    roothash::Message::Staking(Versioned::new(
                        0,
                        roothash::StakingMessage::Transfer(staking::Transfer::default()),
                    )),
                    MessageEventHookInvocation::new("test".to_string(), ""),
                )
                .expect("message emission should succeed");
            assert_eq!(state.emitted_messages_count(), 2);
            assert_eq!(state.emitted_messages_local_count(), 1);
            assert_eq!(state.emitted_messages_max(&ctx), max_messages as u32);

            state.rollback(); // Rollback.

            assert_eq!(
                state.emitted_messages_count(),
                1,
                "emitted message should have been rolled back"
            );
            assert_eq!(state.emitted_messages_local_count(), 1);

            state.open(); // Start child state.

            assert_eq!(state.emitted_messages_local_count(), 0);

            state
                .emit_message(
                    &ctx,
                    roothash::Message::Staking(Versioned::new(
                        0,
                        roothash::StakingMessage::Transfer(staking::Transfer::default()),
                    )),
                    MessageEventHookInvocation::new("test".to_string(), ""),
                )
                .expect("message emission should succeed");
            assert_eq!(state.emitted_messages_count(), 2);
            assert_eq!(state.emitted_messages_local_count(), 1);

            state.commit(); // Commit.

            assert_eq!(
                state.emitted_messages_count(),
                2,
                "emitted message should have been committed"
            );

            // Emit some more messages.
            for _ in 0..max_messages - 2 {
                state
                    .emit_message(
                        &ctx,
                        roothash::Message::Staking(Versioned::new(
                            0,
                            roothash::StakingMessage::Transfer(staking::Transfer::default()),
                        )),
                        MessageEventHookInvocation::new("test".to_string(), ""),
                    )
                    .expect("message emission should succeed");
            }
            assert_eq!(state.emitted_messages_count(), max_messages);

            // Emitting one more message should be rejected.
            state
                .emit_message(
                    &ctx,
                    roothash::Message::Staking(Versioned::new(
                        0,
                        roothash::StakingMessage::Transfer(staking::Transfer::default()),
                    )),
                    MessageEventHookInvocation::new("test".to_string(), ""),
                )
                .expect_err("message emission should fail due to out of slots");
            assert_eq!(state.emitted_messages_count(), max_messages);

            state.rollback(); // Rollback.

            assert_eq!(state.emitted_messages_count(), 0);
        });

        // Change the maximum amount of messages.
        let mut tx = mock::transaction();
        tx.auth_info.fee.consensus_messages = 1; // Limit amount of messages.
        CurrentState::with_transaction_opts(Options::new().with_tx(tx.into()), || {
            CurrentState::with(|state| {
                assert_eq!(state.emitted_messages_max(&ctx), 1);
            });
        });

        let mut tx = mock::transaction();
        tx.auth_info.fee.consensus_messages = 0; // Zero means an implicit limit by gas use.
        CurrentState::with_transaction_opts(Options::new().with_tx(tx.into()), || {
            CurrentState::with(|state| {
                assert_eq!(state.emitted_messages_max(&ctx), max_messages as u32);
            });
        });
    }

    #[test]
    fn test_emit_events() {
        CurrentState::init_local_fallback();

        CurrentState::with(|state| {
            state.open();

            state.open();

            state.emit_event(Event::GasUsed { amount: 41 });
            state.emit_event(Event::GasUsed { amount: 42 });
            state.emit_event(Event::GasUsed { amount: 43 });

            state.emit_unconditional_event(Event::GasUsed { amount: 10 });

            state.commit();

            let events = state.take_events();
            assert_eq!(events.len(), 1, "events should have been propagated");
            let event_key = b"core\x00\x00\x00\x01".to_vec();
            assert_eq!(events[&event_key].len(), 3);

            let events = state.take_unconditional_events();
            assert_eq!(
                events.len(),
                1,
                "unconditional events should have been propagated"
            );
            let event_key = b"core\x00\x00\x00\x01".to_vec();
            assert_eq!(events[&event_key].len(), 1);

            state.emit_event(Event::GasUsed { amount: 41 });
            state.emit_event(Event::GasUsed { amount: 42 });
            state.emit_event(Event::GasUsed { amount: 43 });

            state.emit_unconditional_event(Event::GasUsed { amount: 20 });

            state.rollback();

            let events = state.take_events();
            assert_eq!(events.len(), 0, "events should not have been propagated");

            let events = state.take_unconditional_events();
            assert_eq!(
                events.len(),
                0,
                "unconditional events should not have been propagated"
            );
        });
    }

    fn test_store_basic() {
        CurrentState::start_transaction();

        assert!(
            !CurrentState::with(|state| state.has_pending_store_updates()),
            "should not have pending updates"
        );

        CurrentState::with_store(|store| {
            store.insert(b"test", b"value");
        });

        assert!(
            CurrentState::with(|state| state.has_pending_store_updates()),
            "should have pending updates after insert"
        );

        // Transaction helper.
        CurrentState::with_transaction(|| {
            assert!(
                !CurrentState::with(|state| state.has_pending_store_updates()),
                "should not have pending updates"
            );

            CurrentState::with_store(|store| {
                store.insert(b"test", b"b0rken");
            });

            assert!(
                CurrentState::with(|state| state.has_pending_store_updates()),
                "should have pending updates after insert"
            );

            TransactionResult::Rollback(())
        });

        // Transaction helper with options.
        CurrentState::with_transaction_opts(
            Options::new()
                .with_mode(Mode::Check)
                .with_internal(true)
                .with_tx(TransactionWithMeta {
                    data: mock::transaction(),
                    size: 888,
                    index: 42,
                    hash: Default::default(),
                }),
            || {
                CurrentState::with_env(|env| {
                    assert!(env.is_check_only(), "environment should be updated");
                    assert!(env.is_internal(), "environment should be updated");
                    assert!(env.is_transaction(), "environment should be updated");
                    assert_eq!(env.tx_index(), 42, "environment should be updated");
                    assert_eq!(env.tx_size(), 888, "environment should be updated");
                });

                CurrentState::with_env_origin(|env_origin| {
                    assert!(
                        !env_origin.is_check_only(),
                        "origin environment should be correct"
                    );
                    assert!(
                        !env_origin.is_transaction(),
                        "origin environment should be correct"
                    );
                });

                CurrentState::with_transaction(|| {
                    // Check environment propagation.
                    CurrentState::with_env(|env| {
                        assert!(env.is_check_only(), "environment should propagate");
                        assert!(env.is_internal(), "environment should propagate");
                        assert!(env.is_transaction(), "environment should propagate");
                        assert_eq!(env.tx_index(), 42, "environment should propagate");
                        assert_eq!(env.tx_size(), 888, "environment should propagate");
                    });

                    TransactionResult::Rollback(())
                });

                TransactionResult::Rollback(())
            },
        );

        CurrentState::with_env(|env| {
            assert!(!env.is_transaction(), "environment should not leak");
        });

        // Nested entering, but with a different store.
        let unrelated = mkvs::OverlayTree::new(
            mkvs::Tree::builder()
                .with_root_type(mkvs::RootType::State)
                .build(Box::new(mkvs::sync::NoopReadSyncer)),
        );
        let mut unrelated = MKVSStore::new(unrelated);

        CurrentState::enter(&mut unrelated, || {
            CurrentState::start_transaction();

            CurrentState::with_store(|store| {
                store.insert(b"test", b"should not touch the original root");
            });

            CurrentState::commit_transaction();
        });

        CurrentState::with_store(|store| {
            store.insert(b"another", b"value 2");
        });

        CurrentState::commit_transaction();
    }

    #[test]
    fn test_basic() {
        let root = mkvs::OverlayTree::new(
            mkvs::Tree::builder()
                .with_root_type(mkvs::RootType::State)
                .build(Box::new(mkvs::sync::NoopReadSyncer)),
        );
        let mut root = MKVSStore::new(root);

        CurrentState::enter(&mut root, || {
            test_store_basic();
        });

        let value = root.get(b"test").unwrap();
        assert_eq!(value, b"value");
    }

    #[test]
    fn test_local_fallback() {
        // Initialize the local fallback store.
        CurrentState::init_local_fallback();
        CurrentState::init_local_fallback(); // Should be no-op.

        // Test the basic store -- note, no need to enter as fallback current store is available.
        test_store_basic();

        CurrentState::with_store(|store| {
            let value = store.get(b"test").unwrap();
            assert_eq!(value, b"value");
        });

        // It should be possible to override the fallback by entering explicitly.
        let root = mkvs::OverlayTree::new(
            mkvs::Tree::builder()
                .with_root_type(mkvs::RootType::State)
                .build(Box::new(mkvs::sync::NoopReadSyncer)),
        );
        let mut root = MKVSStore::new(root);

        CurrentState::enter(&mut root, || {
            CurrentState::with_store(|store| {
                assert!(store.get(b"test").is_none(), "store should be empty");
                store.insert(b"unrelated", b"unrelated");
            });

            test_store_basic();
        });

        let value = root.get(b"test").unwrap();
        assert_eq!(value, b"value");
        let value = root.get(b"unrelated").unwrap();
        assert_eq!(value, b"unrelated");

        // Changes should not leak to fallback store.
        CurrentState::with_store(|store| {
            assert!(store.get(b"unrelated").is_none(), "changes should not leak");
        });
    }

    #[test]
    #[should_panic(expected = "must enter context")]
    fn test_fail_not_entered() {
        test_store_basic(); // Should panic due to no current store being available.
    }

    #[test]
    #[should_panic(expected = "must not re-enter with")]
    fn test_fail_reenter_with() {
        CurrentState::init_local_fallback();

        CurrentState::with(|_| {
            CurrentState::with(|_| {
                // Should panic.
            });
        });
    }

    #[test]
    #[should_panic(expected = "must not re-enter with")]
    fn test_fail_reenter_with_start_transaction() {
        CurrentState::init_local_fallback();

        CurrentState::with(|_| {
            CurrentState::start_transaction(); // Should panic.
        });
    }

    #[test]
    #[should_panic(expected = "must not re-enter with")]
    fn test_fail_reenter_with_commit_transaction() {
        CurrentState::init_local_fallback();

        CurrentState::with(|_| {
            CurrentState::commit_transaction(); // Should panic.
        });
    }

    #[test]
    #[should_panic(expected = "must not re-enter with")]
    fn test_fail_reenter_with_rollback_transaction() {
        CurrentState::init_local_fallback();

        CurrentState::with(|_| {
            CurrentState::rollback_transaction(); // Should panic.
        });
    }

    #[test]
    #[should_panic(expected = "must not re-enter from with block")]
    fn test_fail_reenter_with_enter() {
        CurrentState::init_local_fallback();

        CurrentState::with(|_| {
            let unrelated = mkvs::OverlayTree::new(
                mkvs::Tree::builder()
                    .with_root_type(mkvs::RootType::State)
                    .build(Box::new(mkvs::sync::NoopReadSyncer)),
            );
            let mut unrelated = MKVSStore::new(unrelated);

            CurrentState::enter(&mut unrelated, || {
                // Should panic.
            });
        });
    }

    #[test]
    #[should_panic(expected = "must not re-enter from with block")]
    fn test_fail_local_fallback_within_with() {
        let root = mkvs::OverlayTree::new(
            mkvs::Tree::builder()
                .with_root_type(mkvs::RootType::State)
                .build(Box::new(mkvs::sync::NoopReadSyncer)),
        );
        let mut root = MKVSStore::new(root);

        CurrentState::enter(&mut root, || {
            CurrentState::with(|_| {
                CurrentState::init_local_fallback(); // Should panic.
            })
        });
    }

    #[test]
    #[should_panic(expected = "must have no prior states attached to local thread")]
    fn test_fail_local_fallback_within_enter() {
        let root = mkvs::OverlayTree::new(
            mkvs::Tree::builder()
                .with_root_type(mkvs::RootType::State)
                .build(Box::new(mkvs::sync::NoopReadSyncer)),
        );
        let mut root = MKVSStore::new(root);

        CurrentState::enter(&mut root, || {
            CurrentState::init_local_fallback(); // Should panic.
        });
    }

    #[test]
    #[should_panic(expected = "cannot commit on root state")]
    fn test_fail_commit_transaction_must_exist() {
        CurrentState::init_local_fallback();

        CurrentState::commit_transaction(); // Should panic.
    }

    #[test]
    #[should_panic(expected = "cannot rollback on root state")]
    fn test_fail_rollback_transaction_must_exist() {
        CurrentState::init_local_fallback();

        CurrentState::rollback_transaction(); // Should panic.
    }
}
