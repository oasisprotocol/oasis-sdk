use revm::{
    database::DBErrorMarker,
    primitives::{Address, B256, KECCAK_EMPTY, U256},
    state::{Account, AccountInfo, Bytecode},
    Database, DatabaseCommit,
};
use std::{collections::HashMap, error::Error, fmt, vec::Vec};

use std::marker::PhantomData;

use oasis_runtime_sdk::{
    context::Context, core::common::crypto::hash::Hash, modules::accounts::API as _,
    state::CurrentState, types::token, Runtime,
};

use crate::{state, types, Config};

pub struct OasisDB<'ctx, C: Context, Cfg: Config> {
    ctx: &'ctx C,
    _cfg: PhantomData<Cfg>,
    origin: Address,
    origin_nonce_incremented: bool,
}

impl<'ctx, C: Context, Cfg: Config> OasisDB<'ctx, C, Cfg> {
    pub fn new(ctx: &'ctx C, origin: Address) -> Self {
        Self {
            ctx,
            _cfg: PhantomData,
            origin,
            origin_nonce_incremented: false,
        }
    }
}

#[derive(Debug)]
pub struct DBError(pub String);

impl DBErrorMarker for DBError {}
impl Error for DBError {}

impl fmt::Display for DBError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for DBError {
    fn from(s: String) -> Self {
        Self(s)
    }
}

// Implement read-only parts of the database.
impl<'ctx, C: Context, Cfg: Config> Database for OasisDB<'ctx, C, Cfg> {
    type Error = DBError;

    /// Get basic account information.
    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        // Derive SDK account address from the Ethereum address.
        let sdk_address = Cfg::map_address(address);

        print!("*** {:#?}", address);

        // Fetch balance and nonce from SDK accounts. Note that these can never fail.
        let balance =
            <C::Runtime as Runtime>::Accounts::get_balance(sdk_address, Cfg::TOKEN_DENOMINATION)
                .unwrap();
        let mut nonce = <C::Runtime as Runtime>::Accounts::get_nonce(sdk_address).unwrap();

        // If this is the caller's address, the caller nonce has not yet been incremented
        // based on the EVM semantics and this is not a simulation context, return the
        // nonce decremented by one to cancel out the Oasis SDK nonce changes.
        // https://github.com/oasisprotocol/oasis-sdk/commit/eda6e0d67c2b2664182a0d60408875af32562a7f
        let is_simulation = CurrentState::with_env(|env| env.is_simulation());
        if address == self.origin && !self.origin_nonce_incremented && !is_simulation {
            nonce = nonce.saturating_sub(1);
            print!(" ! ");
        }

        // Fetch code for this address from storage.
        let code = CurrentState::with_store(|store| {
            let codes = state::codes(store);

            if let Some(code) = codes.get::<_, Vec<u8>>(address) {
                if !code.is_empty() {
                    Some(Bytecode::new_raw(code.into()))
                } else {
                    None
                }
            } else {
                None
            }
        });

        // Calculate hash of code if it exists.
        let code_hash = match code {
            None => KECCAK_EMPTY,
            Some(ref bc) => bc.hash_slow(),
        };

        println!(": {:#?} {:#?}", balance, nonce);

        Ok(Some(AccountInfo {
            nonce,
            balance: U256::from(balance),
            code,
            code_hash,
        }))
    }

    /// Get account code by its hash (unimplemented).
    fn code_by_hash(&mut self, _code_hash: B256) -> Result<Bytecode, Self::Error> {
        println!("###### code_by_hash called ######");
        Err("getting code by hash is not supported".to_string().into())
    }

    /// Get storage value of address at index.
    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        let address: types::H160 = address.into_array().into();
        let index: types::H256 = index.to_be_bytes().into();

        let res: types::H256 = state::with_storage::<Cfg, _, _, _>(self.ctx, &address, |store| {
            store.get(index).unwrap_or_default()
        });
        Ok(U256::from_be_bytes(res.into()))
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

// Implement committing.
impl<'ctx, C: Context, Cfg: Config> DatabaseCommit for OasisDB<'ctx, C, Cfg> {
    fn commit(&mut self, changes: HashMap<Address, Account>) {
        for (address, account) in changes {
            if !account.is_touched() {
                continue;
            }

            // Derive SDK account address from the Ethereum address.
            let sdk_address = Cfg::map_address(address);

            println!(
                "### {:#?}: {:?} {:?}",
                address, account.info.balance, account.info.nonce
            );

            // Update account's balance, nonce, and code (if any).
            <C::Runtime as Runtime>::Accounts::set_balance(
                sdk_address,
                &token::BaseUnits::new(account.info.balance.to::<u128>(), Cfg::TOKEN_DENOMINATION),
            );

            // XXX
            //<C::Runtime as Runtime>::Accounts::set_nonce(sdk_address, account.info.nonce);

            // XXX: This is probably not the right place to put this...
            let is_simulation = CurrentState::with_env(|env| env.is_simulation());
            if address == self.origin && !is_simulation {
                self.origin_nonce_incremented = true;
            }

            if account.info.code.is_some() {
                let code = account.info.code.unwrap().bytecode().to_vec();
                CurrentState::with_store(|store| {
                    let mut codes = state::codes(store);
                    if !code.is_empty() {
                        codes.insert(address, code);
                    } else {
                        codes.remove(address);
                    }
                });
            } else {
                CurrentState::with_store(|store| {
                    let mut codes = state::codes(store);
                    codes.remove(address);
                });
            }

            // Apply account's storage changes.
            let storage_changes = account
                .storage
                .into_iter()
                .map(|(key, value)| (key, value.present_value()));
            for (key, value) in storage_changes {
                let index: types::H256 = key.to_be_bytes().into();
                let val: types::H256 = value.to_be_bytes().into();

                if value == U256::default() {
                    state::with_storage::<Cfg, _, _, _>(
                        self.ctx,
                        &address.into_array().into(),
                        |store| {
                            store.remove(index);
                        },
                    );
                } else {
                    state::with_storage::<Cfg, _, _, _>(
                        self.ctx,
                        &address.into_array().into(),
                        |store| {
                            store.insert(index, val);
                        },
                    );
                }
            }
        }
    }
}
