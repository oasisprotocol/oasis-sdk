//! EVM backend.
use std::{cell::RefCell, marker::PhantomData};

use evm::backend::{Apply, Backend as EVMBackend, Basic, Log};

use oasis_runtime_sdk::{
    core::common::crypto::hash::Hash,
    modules::{accounts::API as _, core::API as _},
    types::token,
    Context, Runtime,
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

/// This macro is like `fn with_storage(ctx, addr, f: FnOnce(impl Storage) -> T) ->T`
/// that chooses public/confidential storage, if that such a function were possible to
/// write without the compiler complaining about unspecified generic type errors.
macro_rules! with_storage {
    ($ctx:expr, $addr:expr, |$store:ident| $handler:expr) => {
        if Cfg::CONFIDENTIAL {
            #[allow(unused_mut)]
            let mut $store = state::confidential_storage($ctx, $addr);
            $handler
        } else {
            #[allow(unused_mut)]
            let mut $store = state::public_storage($ctx, $addr);
            $handler
        }
    };
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

        if let Some(hash) = block_hashes.get::<_, Hash>(&number.low_u64().to_be_bytes()) {
            primitive_types::H256::from_slice(hash.as_ref())
        } else {
            primitive_types::H256::default()
        }
    }

    fn block_number(&self) -> primitive_types::U256 {
        self.ctx.borrow().runtime_header().round.into()
    }

    fn block_coinbase(&self) -> primitive_types::H160 {
        // Does not make sense in runtime context.
        primitive_types::H160::default()
    }

    fn block_timestamp(&self) -> primitive_types::U256 {
        self.ctx.borrow().runtime_header().timestamp.into()
    }

    fn block_difficulty(&self) -> primitive_types::U256 {
        // Does not make sense in runtime context.
        primitive_types::U256::zero()
    }

    fn block_gas_limit(&self) -> primitive_types::U256 {
        <C::Runtime as Runtime>::Core::max_batch_gas(&mut self.ctx.borrow_mut()).into()
    }

    fn block_base_fee_per_gas(&self) -> primitive_types::U256 {
        <C::Runtime as Runtime>::Core::min_gas_price(
            &mut self.ctx.borrow_mut(),
            &Cfg::TOKEN_DENOMINATION,
        )
        .into()
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
        let res: H256 = with_storage!(*ctx, &address, |store| store.get(&idx).unwrap_or_default());
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

pub(crate) trait EVMBackendExt {
    fn random_bytes(&self, num_words: u64) -> Vec<u8>;
}

impl<T: EVMBackendExt> EVMBackendExt for &T {
    fn random_bytes(&self, num_words: u64) -> Vec<u8> {
        (*self).random_bytes(num_words)
    }
}

impl<'ctx, C: Context, Cfg: Config> EVMBackendExt for Backend<'ctx, C, Cfg> {
    fn random_bytes(&self, num_words: u64) -> Vec<u8> {
        if num_words > 64 {
            // Refuse to generate more than 2 KiB in one go.
            // EVM memory gas is checked only before and after calls, so we won't
            // see the quadratic memory cost until after this call uses its time.
            return vec![];
        }
        let mut ctx = self.ctx.borrow_mut();
        let num_bytes = num_words.checked_mul(32).unwrap_or_default();
        ctx.rng()
            .ok()
            .and_then(|rng| {
                let mut rand_bytes = vec![0u8; num_bytes as usize /* bounds checked above */];
                rand_core::RngCore::try_fill_bytes(rng, &mut rand_bytes).ok()?;
                Some(rand_bytes)
            })
            .unwrap_or_default()
    }
}

/// EVM backend that can apply changes and return an exit value.
pub trait ApplyBackendResult {
    /// Apply given values and logs at backend and return an exit value.
    fn apply<A, I, L>(&mut self, values: A, logs: L) -> evm::ExitReason
    where
        A: IntoIterator<Item = Apply<I>>,
        I: IntoIterator<Item = (primitive_types::H256, primitive_types::H256)>,
        L: IntoIterator<Item = Log>;
}

impl<'c, C: Context, Cfg: Config> ApplyBackendResult for Backend<'c, C, Cfg> {
    fn apply<A, I, L>(&mut self, values: A, logs: L) -> evm::ExitReason
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
                Apply::Delete { .. } => {
                    // Apply::Delete indicates a SELFDESTRUCT action which is not supported.
                    // This assumes that Apply::Delete is ALWAYS and ONLY invoked in SELFDESTRUCT opcodes, which indeed is the case:
                    // https://github.com/rust-blockchain/evm/blob/0fbde9fa7797308290f89111c6abe5cee55a5eac/runtime/src/eval/system.rs#L258-L267
                    //
                    // NOTE: We cannot just check the executors ExitReason if the reason was suicide,
                    //       because that doesn't work in case of cross-contract suicide calls, as only
                    //       the top-level exit reason is returned.
                    return evm::ExitFatal::Other("SELFDESTRUCT not supported".into()).into();
                }
                Apply::Modify {
                    address,
                    basic,
                    code,
                    storage,
                    reset_storage: _,
                } => {
                    // Reset storage is ignored since storage cannot be efficiently reset as this
                    // would require iterating over all of the storage keys. This is fine as reset_storage
                    // is only ever called on non-empty storage when doing SELFDESTRUCT, which we don't support.

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
                    let nonce = basic.nonce.low_u64();
                    if !is_simulation {
                        let old_nonce = Cfg::Accounts::get_nonce(&mut state, address).unwrap();

                        if addr == origin {
                            // Origin's nonce must stay the same as we cancelled out the changes. Note
                            // that in reality this means that the nonce has been incremented by one.
                            assert!(nonce == old_nonce,
                                "evm execution would not increment origin nonce correctly ({} -> {})", old_nonce, nonce);
                        } else {
                            // Other nonces must either stay the same or increment.
                            assert!(nonce >= old_nonce,
                                "evm execution would not update non-origin nonce correctly ({} -> {})", old_nonce, nonce);
                        }
                    }
                    Cfg::Accounts::set_nonce(&mut state, address, nonce);

                    // Handle code updates.
                    if let Some(code) = code {
                        let state = self.ctx.get_mut().runtime_state();
                        let mut store = state::codes(state);
                        store.insert(&addr, code);
                    }

                    // Handle storage updates.
                    for (index, value) in storage {
                        let idx: H256 = index.into();
                        let val: H256 = value.into();

                        let ctx = self.ctx.get_mut();
                        if value == primitive_types::H256::default() {
                            with_storage!(*ctx, &addr, |store| store.remove(&idx));
                        } else {
                            with_storage!(*ctx, &addr, |store| store.insert(&idx, val));
                        }
                    }
                }
            }
        }

        // NOTE: This should never happen and if it does it would cause an invariant violation
        //       so we better abort to avoid corrupting state.
        assert!(
            total_supply_add == total_supply_sub,
            "evm execution would lead to invariant violation ({} != {})",
            total_supply_add,
            total_supply_sub
        );

        // Emit logs as events.
        for log in logs {
            self.ctx.get_mut().emit_event(crate::Event::Log {
                address: log.address.into(),
                topics: log.topics.iter().map(|&topic| topic.into()).collect(),
                data: log.data,
            });
        }

        evm::ExitSucceed::Returned.into()
    }
}
