use std::{
    collections::{btree_map::Entry, BTreeMap, BTreeSet},
    marker::PhantomData,
    mem,
};

use primitive_types::{H160, H256, U256};
use sha3::{Digest as _, Keccak256};

use oasis_runtime_sdk::{
    context::Context,
    core::common::crypto::hash::Hash,
    modules::{accounts::API as _, core::API as _},
    state::CurrentState,
    types::token,
    Runtime,
};

use crate::{state, types, Config, Error};

/// Backend for the evm crate that enables the use of our storage.
pub struct OasisBackend<'ctx, C: Context, Cfg: Config> {
    caller: H160,
    ctx: &'ctx C,
    substate: Box<Substate>,
    original_storage: BTreeMap<(types::H160, types::H256), types::H256>,
    _cfg: PhantomData<Cfg>,
}

impl<'ctx, C: Context, Cfg: Config> OasisBackend<'ctx, C, Cfg> {
    pub fn new(ctx: &'ctx C, caller: H160) -> Self {
        Self {
            caller,
            ctx,
            substate: Box::new(Substate::new()),
            original_storage: BTreeMap::new(),
            _cfg: PhantomData,
        }
    }

    pub fn apply(self) -> Result<(), Error> {
        // Detect use of SELFDESTRUCT and return an error in this case.
        if !self.substate.deletes.is_empty() {
            return Err(Error::ExecutionFailed(
                "SELFDESTRUCT not supported".to_owned(),
            ));
        }

        // Emit logs as events.
        CurrentState::with(|state| {
            for log in self.substate.logs {
                state.emit_event(crate::Event::Log {
                    address: log.address.into(),
                    topics: log.topics.iter().map(|&topic| topic.into()).collect(),
                    data: log.data,
                });
            }
        });

        Ok(())
    }
}

impl<'ctx, C: Context, Cfg: Config> evm::backend::RuntimeEnvironment
    for OasisBackend<'ctx, C, Cfg>
{
    fn block_hash(&self, number: U256) -> H256 {
        CurrentState::with_store(|store| {
            let block_hashes = state::block_hashes(store);

            if let Some(hash) = block_hashes.get::<_, Hash>(&number.low_u64().to_be_bytes()) {
                H256::from_slice(hash.as_ref())
            } else {
                H256::default()
            }
        })
    }

    fn block_number(&self) -> U256 {
        self.ctx.runtime_header().round.into()
    }

    fn block_coinbase(&self) -> H160 {
        // Does not make sense in runtime context.
        H160::default()
    }

    fn block_timestamp(&self) -> U256 {
        self.ctx.runtime_header().timestamp.into()
    }

    fn block_difficulty(&self) -> U256 {
        // Does not make sense in runtime context.
        U256::zero()
    }

    fn block_randomness(&self) -> Option<H256> {
        // TODO: Could use our VRF.
        None
    }

    fn block_gas_limit(&self) -> U256 {
        <C::Runtime as Runtime>::Core::max_batch_gas().into()
    }

    fn block_base_fee_per_gas(&self) -> U256 {
        <C::Runtime as Runtime>::Core::min_gas_price(&Cfg::TOKEN_DENOMINATION)
            .unwrap_or_default()
            .into()
    }

    fn blob_base_fee_per_gas(&self) -> U256 {
        // Does not make sense in runtime context.
        U256::zero()
    }

    fn blob_versioned_hash(&self, _index: U256) -> H256 {
        // Does not make sense in runtime context.
        H256::default()
    }

    fn chain_id(&self) -> U256 {
        Cfg::CHAIN_ID.into()
    }
}

impl<'ctx, C: Context, Cfg: Config> evm::backend::RuntimeBaseBackend
    for OasisBackend<'ctx, C, Cfg>
{
    fn balance(&self, address: H160) -> U256 {
        // Derive SDK account address from the Ethereum address.
        let sdk_address = Cfg::map_address(address);
        // Fetch balance from SDK accounts. Note that this can never fail.
        let balance =
            <C::Runtime as Runtime>::Accounts::get_balance(sdk_address, Cfg::TOKEN_DENOMINATION)
                .unwrap();

        balance.into()
    }

    fn code(&self, address: H160) -> Vec<u8> {
        CurrentState::with_store(|store| {
            let store = state::codes(store);
            store.get(address).unwrap_or_default()
        })
    }

    fn code_hash(&self, address: H160) -> H256 {
        if self.exists(address) {
            H256::from_slice(&Keccak256::digest(&self.code(address)[..]))
        } else {
            H256::default()
        }
    }

    fn storage(&self, address: H160, index: H256) -> H256 {
        let address: types::H160 = address.into();
        let index: types::H256 = index.into();

        let res: types::H256 = state::with_storage::<Cfg, _, _, _>(self.ctx, &address, |store| {
            store.get(index).unwrap_or_default()
        });
        res.into()
    }

    fn transient_storage(&self, address: H160, index: H256) -> H256 {
        // TODO: Is this ok?
        self.substate
            .known_transient_storage(address, index)
            .unwrap_or_default()
    }

    fn exists(&self, address: H160) -> bool {
        let is_empty = self.balance(address) == U256::zero()
            && self.nonce(address) == U256::zero()
            && self.code_size(address) == U256::zero();
        !is_empty
    }

    fn nonce(&self, address: H160) -> U256 {
        // Derive SDK account address from the Ethereum address.
        let sdk_address = Cfg::map_address(address);
        // Fetch nonce from SDK accounts. Note that this can never fail.
        let mut nonce = <C::Runtime as Runtime>::Accounts::get_nonce(sdk_address).unwrap();

        // If this is the caller's address, the caller nonce has not yet been incremented based on
        // the EVM semantics and this is not a simulation context, return the nonce decremented by
        // one to cancel out the SDK nonce changes.
        if address == self.caller
            && !self.substate.origin_nonce_incremented
            && !CurrentState::with_env(|env| env.is_simulation())
        {
            nonce = nonce.saturating_sub(1);
        }

        nonce.into()
    }

    fn can_create(&self, address: H160) -> bool {
        self.nonce(address) == U256::zero() && self.code_size(address) == U256::zero()
    }
}

impl<'ctx, C: Context, Cfg: Config> evm::backend::RuntimeBackend for OasisBackend<'ctx, C, Cfg> {
    fn original_storage(&self, address: H160, index: H256) -> H256 {
        if let Some(value) = self.substate.known_original_storage(address, index) {
            value
        } else {
            use evm::backend::RuntimeBaseBackend;

            self.original_storage
                .get(&(address.into(), index.into()))
                .cloned()
                .map(Into::into)
                .unwrap_or_else(|| self.storage(address, index))
        }
    }

    fn created(&self, address: H160) -> bool {
        self.substate.created(address)
    }

    fn deleted(&self, address: H160) -> bool {
        self.substate.deleted(address)
    }

    fn is_cold(&self, address: H160, index: Option<H256>) -> bool {
        let accessed = self
            .substate
            .known_accessed((address, index))
            .unwrap_or(false);
        !accessed
    }

    fn mark_hot(&mut self, address: H160, kind: evm::interpreter::runtime::TouchKind) {
        use evm::interpreter::runtime::TouchKind;

        match kind {
            TouchKind::Access => {
                self.substate.accessed.insert((address, None));
            }
            TouchKind::StateChange | TouchKind::Coinbase => {
                self.substate.touched.insert(address);
            }
        }
    }

    fn mark_storage_hot(&mut self, address: H160, index: H256) {
        self.substate.accessed.insert((address, Some(index)));
    }

    fn set_storage(
        &mut self,
        address: H160,
        index: H256,
        value: H256,
    ) -> Result<(), evm::interpreter::ExitError> {
        let address: types::H160 = address.into();
        let index: types::H256 = index.into();
        let value: types::H256 = value.into();

        // We cache the current value if this is the first time we modify it in the transaction.
        // TODO
        if let Entry::Vacant(e) = self.original_storage.entry((address, index)) {
            let original = state::with_storage::<Cfg, _, _, _>(self.ctx, &address, |store| {
                store.get(index).unwrap_or_default()
            });
            // No need to cache if same value.
            if original != value {
                e.insert(original);
            }
        }

        state::with_storage::<Cfg, _, _, _>(self.ctx, &address, |store| {
            if value == types::H256::default() {
                store.remove(index);
            } else {
                store.insert(index, value);
            }
        });

        Ok(())
    }

    fn set_transient_storage(
        &mut self,
        address: H160,
        index: H256,
        value: H256,
    ) -> Result<(), evm::interpreter::ExitError> {
        self.substate
            .transient_storage
            .insert((address, index), value);
        Ok(())
    }

    fn log(
        &mut self,
        log: evm::interpreter::runtime::Log,
    ) -> Result<(), evm::interpreter::ExitError> {
        self.substate.logs.push(log);
        Ok(())
    }

    fn mark_delete_reset(&mut self, address: H160) {
        self.substate.deletes.insert(address);
    }

    fn mark_create(&mut self, address: H160) {
        self.substate.creates.insert(address);
    }

    fn reset_storage(&mut self, address: H160) {
        self.substate.storage_resets.insert(address);
    }

    fn set_code(
        &mut self,
        address: H160,
        code: Vec<u8>,
        _origin: evm::interpreter::runtime::SetCodeOrigin,
    ) -> Result<(), evm::interpreter::ExitError> {
        CurrentState::with_store(|store| {
            let mut store = state::codes(store);
            store.insert(address, code);
        });
        Ok(())
    }

    fn deposit(&mut self, _target: H160, _value: U256) {
        // NOTE: Direct balance manipulation is not allowed, use `transfer˙ instead.
        //
        // This is used by the invoker to handle fee refunds (already handled by the SDK) and
        // block rewards (ignored).
    }

    fn withdrawal(
        &mut self,
        _source: H160,
        _value: U256,
    ) -> Result<(), evm::interpreter::ExitError> {
        // NOTE: Direct balance manipulation is not allowed, use `transfer˙ instead.
        //
        // This is used by the invoker to handle fee payments (already handled by the SDK).
        Ok(())
    }

    fn transfer(
        &mut self,
        transfer: evm::interpreter::runtime::Transfer,
    ) -> Result<(), evm::interpreter::ExitError> {
        let from = Cfg::map_address(transfer.source);
        let to = Cfg::map_address(transfer.target);
        let amount = transfer
            .value
            .try_into()
            .map_err(|_| evm::interpreter::ExitException::OutOfFund)?;
        let amount = token::BaseUnits::new(amount, Cfg::TOKEN_DENOMINATION);

        <C::Runtime as Runtime>::Accounts::transfer(from, to, &amount)
            .map_err(|_| evm::interpreter::ExitException::OutOfFund.into())
    }

    fn inc_nonce(&mut self, address: H160) -> Result<(), evm::interpreter::ExitError> {
        // Do not increment the origin nonce as that has already been handled by the SDK. But do
        // record that the nonce should be incremented based on EVM semantics so we can adjust any
        // results from the `nonce` method.
        if address == self.caller && !CurrentState::with_env(|env| env.is_simulation()) {
            self.substate.origin_nonce_incremented = true;
            return Ok(());
        }

        let address = Cfg::map_address(address);
        <C::Runtime as Runtime>::Accounts::inc_nonce(address);
        Ok(())
    }
}

impl<'ctx, C: Context, Cfg: Config> evm::backend::TransactionalBackend
    for OasisBackend<'ctx, C, Cfg>
{
    fn push_substate(&mut self) {
        let mut parent = Box::new(Substate::new());
        mem::swap(&mut parent, &mut self.substate);
        self.substate.parent = Some(parent);

        CurrentState::start_transaction();
    }

    fn pop_substate(
        &mut self,
        strategy: evm::MergeStrategy,
    ) -> Result<(), evm::interpreter::ExitError> {
        let mut child = self
            .substate
            .parent
            .take()
            .ok_or(evm::interpreter::ExitError::Fatal(
                evm::interpreter::ExitFatal::UnevenSubstate,
            ))?;
        mem::swap(&mut child, &mut self.substate);
        let child = child;

        match strategy {
            evm::MergeStrategy::Commit => {
                for log in child.logs {
                    self.substate.logs.push(log);
                }
                for address in child.storage_resets {
                    self.substate.storage_resets.insert(address);
                }
                for ((address, key), value) in child.transient_storage {
                    self.substate
                        .transient_storage
                        .insert((address, key), value);
                }
                for address in child.deletes {
                    self.substate.deletes.insert(address);
                }
                for address in child.creates {
                    self.substate.creates.insert(address);
                }
                for address in child.touched {
                    self.substate.touched.insert(address);
                }
                for item in child.accessed {
                    self.substate.accessed.insert(item);
                }
                self.substate.origin_nonce_incremented |= child.origin_nonce_incremented;

                CurrentState::commit_transaction();
            }
            evm::MergeStrategy::Revert | evm::MergeStrategy::Discard => {
                CurrentState::rollback_transaction();
            }
        }

        Ok(())
    }
}

struct Substate {
    parent: Option<Box<Substate>>,
    logs: Vec<evm::interpreter::runtime::Log>,
    storage_resets: BTreeSet<H160>,
    transient_storage: BTreeMap<(H160, H256), H256>,
    deletes: BTreeSet<H160>,
    creates: BTreeSet<H160>,
    touched: BTreeSet<H160>,
    accessed: BTreeSet<(H160, Option<H256>)>,
    origin_nonce_incremented: bool,
}

impl Substate {
    pub fn new() -> Self {
        Self {
            parent: None,
            logs: Vec::new(),
            storage_resets: Default::default(),
            transient_storage: Default::default(),
            deletes: Default::default(),
            creates: Default::default(),
            touched: Default::default(),
            accessed: Default::default(),
            origin_nonce_incremented: false,
        }
    }

    pub fn known_accessed(&self, item: (H160, Option<H256>)) -> Option<bool> {
        if self.accessed.contains(&item) {
            Some(true)
        } else if let Some(parent) = self.parent.as_ref() {
            parent.known_accessed(item)
        } else {
            None
        }
    }

    pub fn known_original_storage(&self, address: H160, _key: H256) -> Option<H256> {
        if self.deletes.contains(&address) {
            None
        } else if self.storage_resets.contains(&address) {
            Some(H256::default())
        } else if let Some(parent) = self.parent.as_ref() {
            parent.known_original_storage(address, _key)
        } else {
            None
        }
    }

    pub fn known_transient_storage(&self, address: H160, key: H256) -> Option<H256> {
        if let Some(value) = self.transient_storage.get(&(address, key)) {
            Some(*value)
        } else if let Some(parent) = self.parent.as_ref() {
            parent.known_transient_storage(address, key)
        } else {
            None
        }
    }

    pub fn deleted(&self, address: H160) -> bool {
        if self.deletes.contains(&address) {
            true
        } else if let Some(parent) = self.parent.as_ref() {
            parent.deleted(address)
        } else {
            false
        }
    }

    pub fn created(&self, address: H160) -> bool {
        if self.creates.contains(&address) {
            true
        } else if let Some(parent) = self.parent.as_ref() {
            parent.created(address)
        } else {
            false
        }
    }
}
