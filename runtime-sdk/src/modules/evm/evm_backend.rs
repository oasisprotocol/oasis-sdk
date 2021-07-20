//! EVM backend.
use std::cell::RefCell;

use serde::{Deserialize, Serialize};

use crate::storage::{self, Store as _};

use evm::backend::{Apply, ApplyBackend, Backend as EVMBackend, Basic, Log};
use primitive_types::{H160, H256, U256};

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct Vicinity {
    pub gas_price: U256,
    pub origin: H160,
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Default)]
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
    fn gas_price(&self) -> U256 {
        self.vicinity.gas_price
    }
    fn origin(&self) -> H160 {
        self.vicinity.origin
    }
    fn block_hash(&self, _number: U256) -> H256 {
        H256::default()
    }
    fn block_number(&self) -> U256 {
        U256::default()
    }
    fn block_coinbase(&self) -> H160 {
        H160::default()
    }
    fn block_timestamp(&self) -> U256 {
        U256::zero()
    }
    fn block_difficulty(&self) -> U256 {
        U256::zero()
    }
    fn block_gas_limit(&self) -> U256 {
        U256::zero()
    }
    fn chain_id(&self) -> U256 {
        U256::default()
    }
    fn exists(&self, _address: H160) -> bool {
        true
    }

    fn basic(&self, address: H160) -> Basic {
        let mut ctx = self.ctx.borrow_mut();
        let state = ctx.runtime_state();

        let store = storage::PrefixStore::new(state, &crate::modules::evm::MODULE_NAME);
        let accounts = storage::TypedStore::new(storage::PrefixStore::new(store, &ACCOUNTS));

        let a: Account = accounts.get(&address).unwrap_or_default();

        Basic {
            nonce: a.nonce,
            balance: a.balance,
        }
    }

    fn code(&self, address: H160) -> Vec<u8> {
        let mut ctx = self.ctx.borrow_mut();
        let state = ctx.runtime_state();

        let store = storage::PrefixStore::new(state, &crate::modules::evm::MODULE_NAME);
        let codes = storage::TypedStore::new(storage::PrefixStore::new(store, &CODES));

        codes.get(&address).unwrap_or_default()
    }

    fn storage(&self, address: H160, index: H256) -> H256 {
        let mut ctx = self.ctx.borrow_mut();
        let state = ctx.runtime_state();

        let store = storage::PrefixStore::new(state, &crate::modules::evm::MODULE_NAME);
        let storages = storage::PrefixStore::new(store, &STORAGES);
        let s = storage::TypedStore::new(storage::PrefixStore::new(storages, &address));

        s.get(&index).unwrap_or_default()
    }

    fn original_storage(&self, address: H160, index: H256) -> Option<H256> {
        Some(self.storage(address, index))
    }
}

impl<'c, C: crate::Context> ApplyBackend for Backend<'c, C> {
    fn apply<A, I, L>(&mut self, values: A, _logs: L, delete_empty: bool)
    where
        A: IntoIterator<Item = Apply<I>>,
        I: IntoIterator<Item = (H256, H256)>,
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
                    let is_empty = {
                        let state = self.ctx.get_mut().runtime_state();
                        let a_store =
                            storage::PrefixStore::new(state, &crate::modules::evm::MODULE_NAME);
                        let mut accounts =
                            storage::TypedStore::new(storage::PrefixStore::new(a_store, &ACCOUNTS));
                        let mut account: Account = accounts.get(&address).unwrap_or_default();

                        account.balance = basic.balance;
                        account.nonce = basic.nonce;

                        accounts.insert(&address, &account);

                        if let Some(code) = code {
                            let state = self.ctx.get_mut().runtime_state();
                            let c_store =
                                storage::PrefixStore::new(state, &crate::modules::evm::MODULE_NAME);
                            let mut codes = storage::TypedStore::new(storage::PrefixStore::new(
                                c_store, &CODES,
                            ));
                            codes.insert(&address, &code);
                        }

                        if reset_storage {
                            let state = self.ctx.get_mut().runtime_state();
                            let s_store =
                                storage::PrefixStore::new(state, &crate::modules::evm::MODULE_NAME);
                            let mut storages = storage::PrefixStore::new(s_store, &STORAGES);
                            storages.remove(&address);
                        }

                        for (index, value) in storage {
                            let state = self.ctx.get_mut().runtime_state();
                            let s_store =
                                storage::PrefixStore::new(state, &crate::modules::evm::MODULE_NAME);
                            let storages = storage::PrefixStore::new(s_store, &STORAGES);
                            let mut s = storage::TypedStore::new(storage::PrefixStore::new(
                                storages, &address,
                            ));
                            if value == H256::default() {
                                s.remove(&index);
                            } else {
                                s.insert(&index, &value);
                            }
                        }

                        account.balance == U256::zero() && account.nonce == U256::zero()
                    };

                    if is_empty && delete_empty {
                        let state = self.ctx.get_mut().runtime_state();
                        let a2_store =
                            storage::PrefixStore::new(state, &crate::modules::evm::MODULE_NAME);
                        let mut accounts = storage::TypedStore::new(storage::PrefixStore::new(
                            a2_store, &ACCOUNTS,
                        ));

                        accounts.remove(&address);
                    }
                }
                Apply::Delete { address } => {
                    let state = self.ctx.get_mut().runtime_state();
                    let store = storage::PrefixStore::new(state, &crate::modules::evm::MODULE_NAME);
                    let mut accounts =
                        storage::TypedStore::new(storage::PrefixStore::new(store, &ACCOUNTS));

                    accounts.remove(&address);
                }
            }
        }

        // TODO: What to do with logs, emit them as events?
    }
}
