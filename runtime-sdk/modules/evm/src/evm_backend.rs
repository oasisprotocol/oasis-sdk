//! EVM backend.
use std::cell::RefCell;

use evm::backend::{Apply, ApplyBackend, Backend as EVMBackend, Basic, Log};

use oasis_runtime_sdk::{
    core::common::crypto::hash::Hash,
    crypto,
    storage::{self, Store as _},
};

use crate::{
    state,
    types::{H160, H256, U256},
};

/// EVM chain domain separation context.
const EVM_CHAIN_CONTEXT: &[u8] = b"oasis-runtime-sdk/evm: chain id";

/// Information required by the evm crate.
#[derive(Clone, Default, PartialEq, Eq, cbor::Encode, cbor::Decode)]
pub struct Vicinity {
    pub gas_price: U256,
    pub origin: H160,
}

/// Details specific to Ethereum accounts.  Information managed by SDK modules
/// are held by the respective modules (e.g. core).
#[derive(Clone, Default, PartialEq, Eq, cbor::Encode, cbor::Decode)]
pub struct Account {
    pub nonce: U256,
    pub balance: U256,
}

/// Backend for the evm crate that enables the use of our storage.
pub struct Backend<'c, C: oasis_runtime_sdk::Context> {
    vicinity: Vicinity,
    ctx: RefCell<&'c mut C>,
}

impl<'c, C: oasis_runtime_sdk::Context> Backend<'c, C> {
    pub fn new(vicinity: Vicinity, ctx: &'c mut C) -> Self {
        Self {
            vicinity,
            ctx: RefCell::new(ctx),
        }
    }
}

impl<'c, C: oasis_runtime_sdk::Context> EVMBackend for Backend<'c, C> {
    fn gas_price(&self) -> primitive_types::U256 {
        self.vicinity.gas_price.into()
    }
    fn origin(&self) -> primitive_types::H160 {
        self.vicinity.origin.into()
    }
    fn block_hash(&self, number: primitive_types::U256) -> primitive_types::H256 {
        let mut ctx = self.ctx.borrow_mut();
        let state = ctx.runtime_state();

        let store = storage::PrefixStore::new(state, &crate::MODULE_NAME);
        let hashes = storage::PrefixStore::new(store, &state::BLOCK_HASHES);
        let block_hashes = storage::TypedStore::new(hashes);

        if let Some(hash) = block_hashes.get::<_, Hash>(&number.as_u64().to_be_bytes()) {
            primitive_types::H256::from_slice(hash.as_ref())
        } else {
            primitive_types::H256::default()
        }
    }
    fn block_number(&self) -> primitive_types::U256 {
        self.ctx.borrow().runtime_header().round.into()
    }
    fn block_coinbase(&self) -> primitive_types::H160 {
        primitive_types::H160::default()
    }
    fn block_timestamp(&self) -> primitive_types::U256 {
        self.ctx.borrow().runtime_header().timestamp.into()
    }
    fn block_difficulty(&self) -> primitive_types::U256 {
        primitive_types::U256::zero()
    }
    fn block_gas_limit(&self) -> primitive_types::U256 {
        primitive_types::U256::zero()
    }
    fn chain_id(&self) -> primitive_types::U256 {
        crypto::signature::context::get_chain_context_for(EVM_CHAIN_CONTEXT)[..32].into()
    }
    fn exists(&self, address: primitive_types::H160) -> bool {
        let acct = self.basic(address);

        !(acct.nonce == primitive_types::U256::zero()
            && acct.balance == primitive_types::U256::zero())
    }

    fn basic(&self, address: primitive_types::H160) -> Basic {
        let addr: H160 = address.into();

        let mut ctx = self.ctx.borrow_mut();
        let state = ctx.runtime_state();

        let store = storage::PrefixStore::new(state, &crate::MODULE_NAME);
        let accounts = storage::TypedStore::new(storage::PrefixStore::new(store, &state::ACCOUNTS));

        let a: Account = accounts.get(&addr).unwrap_or_default();

        Basic {
            nonce: a.nonce.into(),
            balance: a.balance.into(),
        }
    }

    fn code(&self, address: primitive_types::H160) -> Vec<u8> {
        let addr: H160 = address.into();

        let mut ctx = self.ctx.borrow_mut();
        let state = ctx.runtime_state();

        let store = storage::PrefixStore::new(state, &crate::MODULE_NAME);
        let codes = storage::TypedStore::new(storage::PrefixStore::new(store, &state::CODES));

        codes.get(&addr).unwrap_or_default()
    }

    fn storage(
        &self,
        address: primitive_types::H160,
        index: primitive_types::H256,
    ) -> primitive_types::H256 {
        let addr: H160 = address.into();
        let idx: H256 = index.into();

        let mut ctx = self.ctx.borrow_mut();
        let state = ctx.runtime_state();

        let store = storage::PrefixStore::new(state, &crate::MODULE_NAME);
        let storages = storage::PrefixStore::new(store, &state::STORAGES);
        let s = storage::TypedStore::new(storage::PrefixStore::new(storages, &addr));

        let res: H256 = s.get(&idx).unwrap_or_default();
        res.into()
    }

    fn original_storage(
        &self,
        address: primitive_types::H160,
        index: primitive_types::H256,
    ) -> Option<primitive_types::H256> {
        Some(self.storage(address, index))
    }
}

impl<'c, C: oasis_runtime_sdk::Context> ApplyBackend for Backend<'c, C> {
    fn apply<A, I, L>(&mut self, values: A, logs: L, delete_empty: bool)
    where
        A: IntoIterator<Item = Apply<I>>,
        I: IntoIterator<Item = (primitive_types::H256, primitive_types::H256)>,
        L: IntoIterator<Item = Log>,
    {
        for apply in values {
            match apply {
                Apply::Modify {
                    address,
                    basic,
                    code,
                    storage,
                    reset_storage,
                } => {
                    let addr: H160 = address.into();
                    let is_empty = {
                        let state = self.ctx.get_mut().runtime_state();
                        let a_store = storage::PrefixStore::new(state, &crate::MODULE_NAME);
                        let mut accounts = storage::TypedStore::new(storage::PrefixStore::new(
                            a_store,
                            &state::ACCOUNTS,
                        ));
                        let mut account: Account = accounts.get(&addr).unwrap_or_default();

                        account.balance = basic.balance.into();
                        account.nonce = basic.nonce.into();

                        accounts.insert(&addr, account.clone());

                        if let Some(code) = code {
                            let state = self.ctx.get_mut().runtime_state();
                            let c_store = storage::PrefixStore::new(state, &crate::MODULE_NAME);
                            let mut codes = storage::TypedStore::new(storage::PrefixStore::new(
                                c_store,
                                &state::CODES,
                            ));
                            codes.insert(&addr, code);
                        }

                        if reset_storage {
                            let state = self.ctx.get_mut().runtime_state();
                            let s_store = storage::PrefixStore::new(state, &crate::MODULE_NAME);
                            let mut storages = storage::PrefixStore::new(s_store, &state::STORAGES);
                            storages.remove(addr.as_bytes());
                        }

                        for (index, value) in storage {
                            let idx: H256 = index.into();
                            let val: H256 = value.into();

                            let state = self.ctx.get_mut().runtime_state();
                            let s_store = storage::PrefixStore::new(state, &crate::MODULE_NAME);
                            let storages = storage::PrefixStore::new(s_store, &state::STORAGES);
                            let mut s = storage::TypedStore::new(storage::PrefixStore::new(
                                storages, &addr,
                            ));
                            if value == primitive_types::H256::default() {
                                s.remove(&idx);
                            } else {
                                s.insert(&idx, val);
                            }
                        }

                        account.balance == primitive_types::U256::zero().into()
                            && account.nonce == primitive_types::U256::zero().into()
                    };

                    if is_empty && delete_empty {
                        let state = self.ctx.get_mut().runtime_state();
                        let a2_store = storage::PrefixStore::new(state, &crate::MODULE_NAME);
                        let mut accounts = storage::TypedStore::new(storage::PrefixStore::new(
                            a2_store,
                            &state::ACCOUNTS,
                        ));

                        accounts.remove(&addr);
                    }
                }
                Apply::Delete { address } => {
                    let addr: H160 = address.into();
                    let state = self.ctx.get_mut().runtime_state();
                    let store = storage::PrefixStore::new(state, &crate::MODULE_NAME);
                    let mut accounts = storage::TypedStore::new(storage::PrefixStore::new(
                        store,
                        &state::ACCOUNTS,
                    ));

                    accounts.remove(&addr);
                }
            }
        }

        // Emit logs as events.
        for log in logs {
            self.ctx.get_mut().emit_event(crate::Event::Log {
                address: log.address.into(),
                topics: log.topics.iter().map(|&topic| topic.into()).collect(),
                data: log.data,
            });
        }
    }
}
