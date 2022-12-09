//! Utilities for testing smart contracts.
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use rand_core::{RngCore as _, SeedableRng as _};
use rand_xorshift::XorShiftRng;

use oasis_contract_sdk_crypto as crypto;
use oasis_runtime_sdk::crypto::signature;

use crate::{
    context::Context,
    env::{Crypto, CryptoError, Env},
    event::Event,
    storage::{ConfidentialStore, PublicStore, Store},
    types::{
        address::Address,
        env::{QueryRequest, QueryResponse},
        event::Event as RawEvent,
        message::Message,
        token, CallFormat, ExecutionContext, InstanceId,
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

impl PublicStore for MockStore {}
impl ConfidentialStore for MockStore {}

/// Mock environment.
#[derive(Clone)]
pub struct MockEnv {
    rng: Arc<Mutex<XorShiftRng>>,
}

impl MockEnv {
    /// Create a new mock environment.
    pub fn new() -> Self {
        Default::default()
    }
}

impl Default for MockEnv {
    fn default() -> Self {
        Self {
            rng: Arc::new(Mutex::new(XorShiftRng::seed_from_u64(0))),
        }
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

    #[cfg(feature = "debug-utils")]
    fn debug_print(&self, msg: &str) {
        eprintln!("{}", msg);
    }
}

impl Crypto for MockEnv {
    fn ecdsa_recover(&self, input: &[u8]) -> [u8; 65] {
        crypto::ecdsa::recover(input).unwrap()
    }

    fn signature_verify_ed25519(&self, key: &[u8], message: &[u8], signature: &[u8]) -> bool {
        let key = if let Ok(key) = signature::ed25519::PublicKey::from_bytes(key) {
            key
        } else {
            return false;
        };
        let sig: signature::Signature = signature.to_vec().into();
        key.verify_raw(message, &sig).is_ok()
    }

    fn signature_verify_secp256k1(&self, key: &[u8], message: &[u8], signature: &[u8]) -> bool {
        let key = if let Ok(key) = signature::secp256k1::PublicKey::from_bytes(key) {
            key
        } else {
            return false;
        };
        let sig: signature::Signature = signature.to_vec().into();
        key.verify_raw(message, &sig).is_ok()
    }

    fn signature_verify_sr25519(
        &self,
        key: &[u8],
        context: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> bool {
        let key = if let Ok(key) = signature::sr25519::PublicKey::from_bytes(key) {
            key
        } else {
            return false;
        };
        let sig: signature::Signature = signature.to_vec().into();
        key.verify(context, message, &sig).is_ok()
    }

    fn x25519_derive_symmetric(&self, public_key: &[u8], private_key: &[u8]) -> [u8; 32] {
        crypto::x25519::derive_symmetric(public_key, private_key).unwrap()
    }

    fn deoxysii_seal(
        &self,
        key: &[u8],
        nonce: &[u8],
        message: &[u8],
        additional_data: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        Ok(crypto::deoxysii::seal(key, nonce, message, additional_data).unwrap())
    }

    fn deoxysii_open(
        &self,
        key: &[u8],
        nonce: &[u8],
        message: &[u8],
        additional_data: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        crypto::deoxysii::open(key, nonce, message, additional_data).map_err(|e| match e {
            crypto::deoxysii::Error::DecryptionFailed => CryptoError::DecryptionFailed,
            _ => panic!("unexpected crypto error"),
        })
    }

    fn random_bytes(&self, _pers: &[u8], dst: &mut [u8]) -> usize {
        self.rng.lock().unwrap().fill_bytes(dst);
        dst.len()
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

    fn is_read_only(&self) -> bool {
        self.ec.read_only
    }

    fn call_format(&self) -> CallFormat {
        self.ec.call_format
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
#[doc(hidden)]
macro_rules! __create_contract {
    ($name:ty) => {};
}
