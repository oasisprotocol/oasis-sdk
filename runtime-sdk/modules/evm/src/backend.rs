//! EVM backend.
use std::{cell::RefCell, marker::PhantomData};

use evm::backend::{Apply, ApplyBackend, Backend as EVMBackend, Basic, Log};

use oasis_runtime_sdk::{
    core::common::crypto::hash::Hash, modules::accounts::API as _, types::token, Context,
};

use crate::{
    state,
    types::{H160, H256, U256},
    Config,
};

/// Information required by the evm crate.
#[derive(Clone, Default, PartialEq, Eq, cbor::Encode, cbor::Decode)]
pub struct Vicinity {
    pub gas_price: U256,
    pub origin: H160,
}

/// Backend for the evm crate that enables the use of our storage.
pub struct Backend<'ctx, C: Context, Cfg: Config> {
    vicinity: Vicinity,
    ctx: RefCell<&'ctx mut C>,
    _cfg: PhantomData<Cfg>,
}

impl<'ctx, C: Context, Cfg: Config> Backend<'ctx, C, Cfg> {
    pub fn new(ctx: &'ctx mut C, vicinity: Vicinity) -> Self {
        Self {
            vicinity,
            ctx: RefCell::new(ctx),
            _cfg: PhantomData,
        }
    }
}

impl<'ctx, C: Context, Cfg: Config> EVMBackend for Backend<'ctx, C, Cfg> {
    fn gas_price(&self) -> primitive_types::U256 {
        self.vicinity.gas_price.into()
    }
    fn origin(&self) -> primitive_types::H160 {
        self.vicinity.origin.into()
    }
    fn block_hash(&self, number: primitive_types::U256) -> primitive_types::H256 {
        let mut ctx = self.ctx.borrow_mut();
        let block_hashes = state::block_hashes(ctx.runtime_state());

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
    fn block_base_fee_per_gas(&self) -> primitive_types::U256 {
        primitive_types::U256::zero()
    }
    fn chain_id(&self) -> primitive_types::U256 {
        Cfg::CHAIN_ID.into()
    }
    fn exists(&self, address: primitive_types::H160) -> bool {
        let acct = self.basic(address);

        !(acct.nonce == primitive_types::U256::zero()
            && acct.balance == primitive_types::U256::zero())
    }

    fn basic(&self, address: primitive_types::H160) -> Basic {
        let mut ctx = self.ctx.borrow_mut();
        let mut state = ctx.runtime_state();

        // Derive SDK account address from the Ethereum address.
        let sdk_address = Cfg::map_address(address);
        // Fetch balance and nonce from SDK accounts. Note that these can never fail.
        let balance =
            Cfg::Accounts::get_balance(&mut state, sdk_address, Cfg::TOKEN_DENOMINATION).unwrap();
        let mut nonce = Cfg::Accounts::get_nonce(&mut state, sdk_address).unwrap();

        // If this is the caller's address and this is not a simulation context, return the nonce
        // decremented by one to cancel out the SDK nonce changes.
        if address == self.origin() && !ctx.is_simulation() {
            // NOTE: This should not overflow as in non-simulation context the nonce should have
            //       been incremented by the authentication handler. Tests should make sure to
            //       either configure simulation mode or set up the nonce correctly.
            nonce -= 1;
        }

        Basic {
            nonce: nonce.into(),
            balance: balance.into(),
        }
    }

    fn code(&self, address: primitive_types::H160) -> Vec<u8> {
        let address: H160 = address.into();

        let mut ctx = self.ctx.borrow_mut();
        let store = state::codes(ctx.runtime_state());
        store.get(&address).unwrap_or_default()
    }

    fn storage(
        &self,
        address: primitive_types::H160,
        index: primitive_types::H256,
    ) -> primitive_types::H256 {
        let address: H160 = address.into();
        let idx: H256 = index.into();

        let mut ctx = self.ctx.borrow_mut();
        let store = state::storage(ctx.runtime_state(), &address);
        let res: H256 = store.get(&idx).unwrap_or_default();
        res.into()
    }

    fn original_storage(
        &self,
        _address: primitive_types::H160,
        _index: primitive_types::H256,
    ) -> Option<primitive_types::H256> {
        None
    }
}

impl<'c, C: Context, Cfg: Config> ApplyBackend for Backend<'c, C, Cfg> {
    fn apply<A, I, L>(&mut self, values: A, logs: L, _delete_empty: bool)
    where
        A: IntoIterator<Item = Apply<I>>,
        I: IntoIterator<Item = (primitive_types::H256, primitive_types::H256)>,
        L: IntoIterator<Item = Log>,
    {
        // Keep track of the total supply change as a paranoid sanity check as it seems to be cheap
        // enough to do (all balances should already be in the storage cache).
        let mut total_supply_add = 0u128;
        let mut total_supply_sub = 0u128;
        // Keep origin handy for nonce sanity checks.
        let origin = self.vicinity.origin;
        let is_simulation = self.ctx.get_mut().is_simulation();

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
                    // Derive SDK account address from the Ethereum address.
                    let address = Cfg::map_address(address);

                    // Update account balance and nonce.
                    let mut state = self.ctx.get_mut().runtime_state();
                    let amount = basic.balance.as_u128();
                    let old_amount =
                        Cfg::Accounts::get_balance(&mut state, address, Cfg::TOKEN_DENOMINATION)
                            .unwrap();
                    if amount > old_amount {
                        total_supply_add =
                            total_supply_add.checked_add(amount - old_amount).unwrap();
                    } else {
                        total_supply_sub =
                            total_supply_sub.checked_add(old_amount - amount).unwrap();
                    }
                    let amount = token::BaseUnits::new(amount, Cfg::TOKEN_DENOMINATION);
                    // Setting the balance like this is dangerous, but we have a sanity check below
                    // to ensure that this never results in any tokens being either minted or
                    // burned.
                    Cfg::Accounts::set_balance(&mut state, address, &amount);

                    // Sanity check nonce updates to make sure that they behave exactly the same as
                    // what we do anyway when authenticating transactions.
                    let nonce = basic.nonce.as_u64();
                    if !is_simulation {
                        let old_nonce = Cfg::Accounts::get_nonce(&mut state, address).unwrap();

                        if addr == origin {
                            // Origin's nonce must stay the same as we cancelled out the changes. Note
                            // that in reality this means that the nonce has been incremented by one.
                            if nonce != old_nonce {
                                panic!("evm execution would not increment origin nonce correctly ({} -> {})", old_nonce, nonce);
                            }
                        } else {
                            // Other nonces must increment by one or stay the same. Note that even
                            // non-origin nonces may increment due to `create_increase_nonce` config.
                            if nonce != old_nonce && nonce != old_nonce + 1 {
                                panic!("evm execution would not update non-origin nonce correctly ({} -> {})", old_nonce, nonce);
                            }
                        }
                    }
                    Cfg::Accounts::set_nonce(&mut state, address, nonce);

                    // Handle code updates.
                    if let Some(code) = code {
                        let state = self.ctx.get_mut().runtime_state();
                        let mut store = state::codes(state);
                        store.insert(&addr, code);
                    }

                    // Handle storage reset.
                    if reset_storage {
                        // NOTE: Storage cannot be efficiently reset as this would require iterating
                        //       over all of the storage keys. We could add this if remove_prefix
                        //       existed.
                    }

                    // Handle storage updates.
                    for (index, value) in storage {
                        let idx: H256 = index.into();
                        let val: H256 = value.into();

                        let mut store = state::storage(self.ctx.get_mut().runtime_state(), &addr);
                        if value == primitive_types::H256::default() {
                            store.remove(&idx);
                        } else {
                            store.insert(&idx, val);
                        }
                    }
                }
                Apply::Delete { .. } => {
                    // Accounts cannot be deleted.
                }
            }
        }

        if total_supply_add != total_supply_sub {
            // NOTE: This should never happen and if it does it would cause an invariant violation
            //       so we better abort to avoid corrupting state.
            panic!(
                "evm execution would lead to invariant violation ({} != {})",
                total_supply_add, total_supply_sub
            );
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
