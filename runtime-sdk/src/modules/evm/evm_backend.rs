//! EVM backend.
use std::cell::RefCell;

use crate::storage::{self, Store as _};

use crate::modules::evm::types::{H160, H256, U256};
use evm::backend::{Apply, ApplyBackend, Backend as EVMBackend, Basic, Log};

#[derive(Clone, Eq, PartialEq, cbor::Encode, cbor::Decode, Default)]
pub struct Vicinity {
    pub gas_price: U256,
    pub origin: H160,
}

#[derive(Clone, Eq, PartialEq, cbor::Encode, cbor::Decode, Default)]
pub struct Account {
    pub nonce: U256,
    pub balance: U256,
}

pub struct Backend<'c, C: crate::Context> {
    vicinity: Vicinity,
    ctx: RefCell<&'c mut C>,
}

impl<'c, C: crate::Context> Backend<'c, C> {
    pub fn new(vicinity: Vicinity, ctx: &'c mut C) -> Self {
        Self {
            vicinity,
            ctx: RefCell::new(ctx),
        }
    }
}

/// Prefix for Ethereum accounts in our storage (maps H160 -> Account).
pub const ACCOUNTS: &[u8] = &[0xea];
/// Prefix for Ethereum account code in our storage (maps H160 -> Vec<u8>).
pub const CODES: &[u8] = &[0xec];
/// Prefix for Ethereum account storage in our storage (maps H160||H256 -> H256).
pub const STORAGES: &[u8] = &[0xe5];

impl<'c, C: crate::Context> EVMBackend for Backend<'c, C> {
    fn gas_price(&self) -> primitive_types::U256 {
        self.vicinity.gas_price.into()
    }
    fn origin(&self) -> primitive_types::H160 {
        self.vicinity.origin.into()
    }
    fn block_hash(&self, _number: primitive_types::U256) -> primitive_types::H256 {
        primitive_types::H256::default()
    }
    fn block_number(&self) -> primitive_types::U256 {
        primitive_types::U256::default()
    }
    fn block_coinbase(&self) -> primitive_types::H160 {
        primitive_types::H160::default()
    }
    fn block_timestamp(&self) -> primitive_types::U256 {
        primitive_types::U256::zero()
    }
    fn block_difficulty(&self) -> primitive_types::U256 {
        primitive_types::U256::zero()
    }
    fn block_gas_limit(&self) -> primitive_types::U256 {
        primitive_types::U256::zero()
    }
    fn chain_id(&self) -> primitive_types::U256 {
        primitive_types::U256::default()
    }
    fn exists(&self, _address: primitive_types::H160) -> bool {
        true
    }

    fn basic(&self, address: primitive_types::H160) -> Basic {
        let addr: H160 = address.into();

        let mut ctx = self.ctx.borrow_mut();
        let state = ctx.runtime_state();

        let store = storage::PrefixStore::new(state, &crate::modules::evm::MODULE_NAME);
        let accounts = storage::TypedStore::new(storage::PrefixStore::new(store, &ACCOUNTS));

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

        let store = storage::PrefixStore::new(state, &crate::modules::evm::MODULE_NAME);
        let codes = storage::TypedStore::new(storage::PrefixStore::new(store, &CODES));

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

        let store = storage::PrefixStore::new(state, &crate::modules::evm::MODULE_NAME);
        let storages = storage::PrefixStore::new(store, &STORAGES);
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

impl<'c, C: crate::Context> ApplyBackend for Backend<'c, C> {
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
                        let a_store =
                            storage::PrefixStore::new(state, &crate::modules::evm::MODULE_NAME);
                        let mut accounts =
                            storage::TypedStore::new(storage::PrefixStore::new(a_store, &ACCOUNTS));
                        let mut account: Account = accounts.get(&addr).unwrap_or_default();

                        account.balance = basic.balance.into();
                        account.nonce = basic.nonce.into();

                        accounts.insert(&addr, account.clone());

                        if let Some(code) = code {
                            let state = self.ctx.get_mut().runtime_state();
                            let c_store =
                                storage::PrefixStore::new(state, &crate::modules::evm::MODULE_NAME);
                            let mut codes = storage::TypedStore::new(storage::PrefixStore::new(
                                c_store, &CODES,
                            ));
                            codes.insert(&addr, code);
                        }

                        if reset_storage {
                            let state = self.ctx.get_mut().runtime_state();
                            let s_store =
                                storage::PrefixStore::new(state, &crate::modules::evm::MODULE_NAME);
                            let mut storages = storage::PrefixStore::new(s_store, &STORAGES);
                            storages.remove(&addr);
                        }

                        for (index, value) in storage {
                            let idx: H256 = index.into();
                            let val: H256 = value.into();

                            let state = self.ctx.get_mut().runtime_state();
                            let s_store =
                                storage::PrefixStore::new(state, &crate::modules::evm::MODULE_NAME);
                            let storages = storage::PrefixStore::new(s_store, &STORAGES);
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
                        let a2_store =
                            storage::PrefixStore::new(state, &crate::modules::evm::MODULE_NAME);
                        let mut accounts = storage::TypedStore::new(storage::PrefixStore::new(
                            a2_store, &ACCOUNTS,
                        ));

                        accounts.remove(&addr);
                    }
                }
                Apply::Delete { address } => {
                    let addr: H160 = address.into();
                    let state = self.ctx.get_mut().runtime_state();
                    let store = storage::PrefixStore::new(state, &crate::modules::evm::MODULE_NAME);
                    let mut accounts =
                        storage::TypedStore::new(storage::PrefixStore::new(store, &ACCOUNTS));

                    accounts.remove(&addr);
                }
            }
        }

        // TODO: What to do with logs, emit them as events?
        for log in logs {
            println!("LOG: {:?}", log);
        }
    }
}
