//! Utilities for testing smart contracts.
use std::collections::BTreeMap;

use oasis_contract_sdk_crypto as crypto;

use crate::{
    context::Context,
    env::{Crypto, Env},
    event::Event,
    storage::Store,
    types::{
        address::Address,
        env::{QueryRequest, QueryResponse},
        event::Event as RawEvent,
        message::Message,
        token, ExecutionContext, InstanceId,
    },
};

/// Mock store.
#[derive(Clone, Default)]
pub struct MockStore {
    inner: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl MockStore {
    /// Create a new empty mock store.
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }
}

impl Store for MockStore {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.inner.get(key).cloned()
    }

    fn insert(&mut self, key: &[u8], value: &[u8]) {
        self.inner.insert(key.to_owned(), value.to_owned());
    }

    fn remove(&mut self, key: &[u8]) {
        self.inner.remove(key);
    }
}

/// Mock environment.
#[derive(Clone, Default)]
pub struct MockEnv {}

impl MockEnv {
    /// Create a new mock environment.
    pub fn new() -> Self {
        Self {}
    }
}

impl Env for MockEnv {
    fn query<Q: Into<QueryRequest>>(&self, query: Q) -> QueryResponse {
        match query.into() {
            QueryRequest::BlockInfo => QueryResponse::BlockInfo {
                round: 42,
                epoch: 2,
                timestamp: 100_000,
            },
            _ => unimplemented!(),
        }
    }

    fn address_for_instance(&self, instance_id: InstanceId) -> Address {
        let b = [
            "test_12345678".as_bytes(),
            &instance_id.as_u64().to_be_bytes(),
        ]
        .concat();
        Address::from_bytes(&b).unwrap()
    }
}

impl Crypto for MockEnv {
    fn ecdsa_recover(&self, input: &[u8]) -> [u8; 65] {
        crypto::ecdsa::recover(input).unwrap()
    }
}

/// A mock contract context suitable for testing.
pub struct MockContext {
    /// Execution context.
    pub ec: ExecutionContext,

    /// Public store.
    pub public_store: MockStore,
    /// "Confidential" store.
    pub confidential_store: MockStore,
    /// Environment.
    pub env: MockEnv,

    /// Emitted messages.
    pub messages: Vec<Message>,
    /// Emitted events.
    pub events: Vec<RawEvent>,
}

impl From<ExecutionContext> for MockContext {
    fn from(ec: ExecutionContext) -> Self {
        Self {
            ec,
            public_store: MockStore::new(),
            confidential_store: MockStore::new(),
            env: MockEnv::new(),
            messages: Vec::new(),
            events: Vec::new(),
        }
    }
}

impl Context for MockContext {
    type PublicStore = MockStore;
    type ConfidentialStore = MockStore;
    type Env = MockEnv;

    fn instance_id(&self) -> InstanceId {
        self.ec.instance_id
    }

    fn instance_address(&self) -> &Address {
        &self.ec.instance_address
    }

    fn caller_address(&self) -> &Address {
        &self.ec.caller_address
    }

    fn deposited_tokens(&self) -> &[token::BaseUnits] {
        &self.ec.deposited_tokens
    }

    fn emit_message(&mut self, msg: Message) {
        self.messages.push(msg);
    }

    fn emit_event<E: Event>(&mut self, event: E) {
        self.events.push(event.into_raw());
    }

    fn public_store(&mut self) -> &mut Self::PublicStore {
        &mut self.public_store
    }

    fn confidential_store(&mut self) -> &mut Self::ConfidentialStore {
        &mut self.confidential_store
    }

    fn env(&self) -> &Self::Env {
        &self.env
    }
}

/// A macro that creates Oasis ABI entry points.
#[macro_export]
macro_rules! create_contract {
    ($name:ty) => {};
}
