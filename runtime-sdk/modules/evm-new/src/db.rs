use revm::{
    primitives::{keccak256, AccountInfo, Address, Bytecode, Log, B256, KECCAK_EMPTY, U256},
    Database,
};
use std::{convert::Infallible, vec::Vec};

use std::marker::PhantomData;

use oasis_runtime_sdk::{
    context::Context,
    core::common::crypto::hash::Hash,
    modules::{
        accounts::API as _,
        core::{self, API as _},
    },
    state::CurrentState,
    subcall,
    types::token,
    Runtime,
};

use crate::{state, types, Config};

pub struct OasisDB<'ctx, C: Context, Cfg: Config> {
    ctx: &'ctx C,
    _cfg: PhantomData<Cfg>,
}

impl<'ctx, C: Context, Cfg: Config> OasisDB<'ctx, C, Cfg> {
    pub fn new(ctx: &'ctx C) -> Self {
        Self {
            ctx,
            _cfg: PhantomData,
        }
    }
}

impl<'ctx, C: Context, Cfg: Config> Database for OasisDB<'ctx, C, Cfg> {
    type Error = Infallible;

    /// Get basic account information.
    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        // Derive SDK account address from the Ethereum address.
        let sdk_address = Cfg::map_address(address);

        // Fetch balance and nonce from SDK accounts. Note that these can never fail.
        let balance =
            <C::Runtime as Runtime>::Accounts::get_balance(sdk_address, Cfg::TOKEN_DENOMINATION)
                .unwrap();
        let mut nonce = <C::Runtime as Runtime>::Accounts::get_nonce(sdk_address).unwrap();

        // Fetch code for this address from storage.
        let code = CurrentState::with_store(|store| {
            let codes = state::codes(store);

            if let Some(code) = codes.get::<_, Vec<u8>>(address) {
                Some(Bytecode::new_raw(code.into()))
            } else {
                None
            }
        });

        // Calculate hash of code if it exists.
        let code_hash = match code {
            None => KECCAK_EMPTY,
            Some(ref bc) => bc.hash_slow(),
        };

        Ok(Some(AccountInfo {
            nonce: nonce.into(),
            balance: U256::from(balance),
            code,
            code_hash,
        }))
    }

    /// Get account code by its hash (unimplemented).
    fn code_by_hash(&mut self, _code_hash: B256) -> Result<Bytecode, Self::Error> {
        // XXX: return an error here instead.
        Ok(Bytecode::new())
    }

    /// Get storage value of address at index.
    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        let address: types::H160 = address.into_array().into();
        let index: types::H256 = index.to_be_bytes().into(); // XXX: is BE ok?

        let res: types::H256 = state::with_storage::<Cfg, _, _, _>(self.ctx, &address, |store| {
            store.get(index).unwrap_or_default()
        });
        Ok(U256::from_be_bytes(res.into())) // XXX: is BE ok?
    }

    /// Get block hash by block number.
    fn block_hash(&mut self, number: u64) -> Result<B256, Self::Error> {
        CurrentState::with_store(|store| {
            let block_hashes = state::block_hashes(store);

            if let Some(hash) = block_hashes.get::<_, Hash>(&number.to_be_bytes()) {
                Ok(B256::from_slice(hash.as_ref()))
            } else {
                Ok(B256::default())
            }
        })
    }
}
