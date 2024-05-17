use std::{
    collections::{btree_map::Entry, BTreeMap, BTreeSet},
    marker::PhantomData,
    mem,
};

use evm::{
    backend::{Backend, Basic, Log},
    executor::stack::{Accessed, StackState, StackSubstateMetadata},
    ExitError, Transfer,
};
use primitive_types::{H160, H256, U256};

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

/// The maximum number of bytes that may be generated by one invocation of
/// [`EVMBackendExt::random_bytes`].
///
/// The precompile function also limits the number of bytes returned, but it's here, too, to prevent
/// accidental memory overconsumption.
///
/// This constant might make a good config param, if anyone asks or this changes frequently.
pub(crate) const RNG_MAX_BYTES: u64 = 1024;

/// Information required by the evm crate.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Vicinity {
    pub gas_price: U256,
    pub origin: H160,
}

/// Backend for the evm crate that enables the use of our storage.
pub struct OasisBackend<'ctx, C: Context, Cfg: Config> {
    vicinity: Vicinity,
    ctx: &'ctx C,
    _cfg: PhantomData<Cfg>,
}

impl<'ctx, C: Context, Cfg: Config> OasisBackend<'ctx, C, Cfg> {
    pub fn new(ctx: &'ctx C, vicinity: Vicinity) -> Self {
        Self {
            vicinity,
            ctx,
            _cfg: PhantomData,
        }
    }
}

/// An extension trait implemented for any [`Backend`].
pub(crate) trait EVMBackendExt {
    /// Returns at most `num_bytes` bytes of cryptographically secure random bytes.
    /// The optional personalization string may be included to increase domain separation.
    fn random_bytes(&self, num_bytes: u64, pers: &[u8]) -> Vec<u8>;

    /// Perform a subcall.
    fn subcall<V: subcall::Validator + 'static>(
        &self,
        info: subcall::SubcallInfo,
        validator: V,
    ) -> Result<subcall::SubcallResult, core::Error>;
}

impl<T: EVMBackendExt> EVMBackendExt for &T {
    fn random_bytes(&self, num_bytes: u64, pers: &[u8]) -> Vec<u8> {
        (*self).random_bytes(num_bytes, pers)
    }

    fn subcall<V: subcall::Validator + 'static>(
        &self,
        info: subcall::SubcallInfo,
        validator: V,
    ) -> Result<subcall::SubcallResult, core::Error> {
        (*self).subcall(info, validator)
    }
}

impl<'ctx, C: Context, Cfg: Config> EVMBackendExt for OasisBackend<'ctx, C, Cfg> {
    fn random_bytes(&self, num_bytes: u64, pers: &[u8]) -> Vec<u8> {
        // Refuse to generate more than 1 KiB in one go.
        // EVM memory gas is checked only before and after calls, so we won't
        // see the quadratic memory cost until after this call uses its time.
        let num_bytes = num_bytes.min(RNG_MAX_BYTES) as usize;
        let mut rand_bytes = vec![0u8; num_bytes];
        CurrentState::with(|state| {
            let mut rng = state
                .rng()
                .fork(self.ctx, pers)
                .expect("unable to access RNG");
            rand_core::RngCore::try_fill_bytes(&mut rng, &mut rand_bytes)
                .expect("RNG is inoperable");
        });

        rand_bytes
    }

    fn subcall<V: subcall::Validator + 'static>(
        &self,
        info: subcall::SubcallInfo,
        validator: V,
    ) -> Result<subcall::SubcallResult, core::Error> {
        subcall::call(self.ctx, info, validator)
    }
}

/// Oasis-specific substate implementation for the EVM stack executor.
///
/// The substate is used to track nested transactional state that can be either be committed or
/// reverted. This is similar to `State` in the SDK.
///
/// See the `evm` crate for details.
struct OasisStackSubstate<'config> {
    metadata: StackSubstateMetadata<'config>,
    parent: Option<Box<OasisStackSubstate<'config>>>,
    logs: Vec<Log>,
    deletes: BTreeSet<H160>,
    origin_nonce_incremented: bool,
}

impl<'config> OasisStackSubstate<'config> {
    fn new(metadata: StackSubstateMetadata<'config>) -> Self {
        Self {
            metadata,
            parent: None,
            logs: Vec::new(),
            deletes: BTreeSet::new(),
            origin_nonce_incremented: false,
        }
    }

    fn metadata(&self) -> &StackSubstateMetadata<'config> {
        &self.metadata
    }

    fn metadata_mut(&mut self) -> &mut StackSubstateMetadata<'config> {
        &mut self.metadata
    }

    fn enter(&mut self, gas_limit: u64, is_static: bool) {
        let mut entering = Self {
            metadata: self.metadata.spit_child(gas_limit, is_static),
            parent: None,
            logs: Vec::new(),
            deletes: BTreeSet::new(),
            origin_nonce_incremented: false,
        };
        mem::swap(&mut entering, self);

        self.parent = Some(Box::new(entering));
    }

    fn exit_commit(&mut self) -> Result<(), ExitError> {
        let mut exited = *self.parent.take().expect("Cannot commit on root substate");
        mem::swap(&mut exited, self);

        self.metadata.swallow_commit(exited.metadata)?;
        self.logs.append(&mut exited.logs);
        self.deletes.append(&mut exited.deletes);
        self.origin_nonce_incremented |= exited.origin_nonce_incremented;

        Ok(())
    }

    fn exit_revert(&mut self) -> Result<(), ExitError> {
        let mut exited = *self.parent.take().expect("Cannot revert on root substate");
        mem::swap(&mut exited, self);

        self.metadata.swallow_revert(exited.metadata)
    }

    fn exit_discard(&mut self) -> Result<(), ExitError> {
        let mut exited = *self.parent.take().expect("Cannot discard on root substate");
        mem::swap(&mut exited, self);

        self.metadata.swallow_discard(exited.metadata)
    }

    fn recursive_is_cold<F: Fn(&Accessed) -> bool>(&self, f: &F) -> bool {
        let local_is_accessed = self.metadata.accessed().as_ref().map(f).unwrap_or(false);
        if local_is_accessed {
            false
        } else {
            self.parent
                .as_ref()
                .map(|p| p.recursive_is_cold(f))
                .unwrap_or(true)
        }
    }

    fn deleted(&self, address: H160) -> bool {
        if self.deletes.contains(&address) {
            return true;
        }

        if let Some(parent) = self.parent.as_ref() {
            return parent.deleted(address);
        }

        false
    }

    fn log(&mut self, address: H160, topics: Vec<H256>, data: Vec<u8>) {
        self.logs.push(Log {
            address,
            topics,
            data,
        });
    }

    fn set_deleted(&mut self, address: H160) {
        self.deletes.insert(address);
    }
}

/// Oasis-specific state implementation for the EVM stack executor.
///
/// The state maintains a hierarchy of nested transactional states (through [`OasisStackSubstate`])
/// and exposes it through accessors to the EVM stack executor.
///
/// See the `evm` crate for details.
pub struct OasisStackState<'ctx, 'backend, 'config, C: Context, Cfg: Config> {
    backend: &'backend OasisBackend<'ctx, C, Cfg>,
    substate: OasisStackSubstate<'config>,
    original_storage: BTreeMap<(types::H160, types::H256), types::H256>,
}

impl<'ctx, 'backend, 'config, C: Context, Cfg: Config>
    OasisStackState<'ctx, 'backend, 'config, C, Cfg>
{
    /// Create a new Oasis-specific state for the EVM stack executor.
    pub fn new(
        metadata: StackSubstateMetadata<'config>,
        backend: &'backend OasisBackend<'ctx, C, Cfg>,
    ) -> Self {
        Self {
            backend,
            substate: OasisStackSubstate::new(metadata),
            original_storage: BTreeMap::new(),
        }
    }

    /// Applies any final state by emitting SDK events/messages.
    ///
    /// Note that storage has already been committed to the top-level current store.
    pub fn apply(self) -> Result<(), crate::Error> {
        // Abort if SELFDESTRUCT was used.
        if !self.substate.deletes.is_empty() {
            return Err(crate::Error::ExecutionFailed(
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

impl<'ctx, 'backend, 'config, C: Context, Cfg: Config> Backend
    for OasisStackState<'ctx, 'backend, 'config, C, Cfg>
{
    fn gas_price(&self) -> U256 {
        self.backend.vicinity.gas_price
    }

    fn origin(&self) -> H160 {
        self.backend.vicinity.origin
    }

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
        self.backend.ctx.runtime_header().round.into()
    }

    fn block_coinbase(&self) -> H160 {
        // Does not make sense in runtime context.
        H160::default()
    }

    fn block_timestamp(&self) -> U256 {
        self.backend.ctx.runtime_header().timestamp.into()
    }

    fn block_difficulty(&self) -> U256 {
        // Does not make sense in runtime context.
        U256::zero()
    }

    fn block_randomness(&self) -> Option<H256> {
        None
    }

    fn block_gas_limit(&self) -> U256 {
        <C::Runtime as Runtime>::Core::max_batch_gas().into()
    }

    fn block_base_fee_per_gas(&self) -> U256 {
        <C::Runtime as Runtime>::Core::min_gas_price(self.backend.ctx, &Cfg::TOKEN_DENOMINATION)
            .unwrap_or_default()
            .into()
    }

    fn chain_id(&self) -> U256 {
        Cfg::CHAIN_ID.into()
    }

    fn exists(&self, address: H160) -> bool {
        let acct = self.basic(address);

        !(acct.nonce == U256::zero() && acct.balance == U256::zero())
    }

    fn basic(&self, address: H160) -> Basic {
        // Derive SDK account address from the Ethereum address.
        let sdk_address = Cfg::map_address(address);
        // Fetch balance and nonce from SDK accounts. Note that these can never fail.
        let balance =
            <C::Runtime as Runtime>::Accounts::get_balance(sdk_address, Cfg::TOKEN_DENOMINATION)
                .unwrap();
        let mut nonce = <C::Runtime as Runtime>::Accounts::get_nonce(sdk_address).unwrap();

        // If this is the caller's address, the caller nonce has not yet been incremented based on
        // the EVM semantics and this is not a simulation context, return the nonce decremented by
        // one to cancel out the SDK nonce changes.
        if address == self.origin()
            && !self.substate.origin_nonce_incremented
            && !CurrentState::with_env(|env| env.is_simulation())
        {
            nonce = nonce.saturating_sub(1);
        }

        Basic {
            nonce: nonce.into(),
            balance: balance.into(),
        }
    }

    fn code(&self, address: H160) -> Vec<u8> {
        CurrentState::with_store(|store| {
            let store = state::codes(store);
            store.get(address).unwrap_or_default()
        })
    }

    fn storage(&self, address: H160, key: H256) -> H256 {
        let address: types::H160 = address.into();
        let key: types::H256 = key.into();

        let res: types::H256 =
            state::with_storage::<Cfg, _, _, _>(self.backend.ctx, &address, |store| {
                store.get(key).unwrap_or_default()
            });
        res.into()
    }

    fn original_storage(&self, address: H160, key: H256) -> Option<H256> {
        Some(
            self.original_storage
                .get(&(address.into(), key.into()))
                .cloned()
                .map(Into::into)
                .unwrap_or_else(|| self.storage(address, key)),
        )
    }
}

impl<'ctx, 'backend, 'config, C: Context, Cfg: Config> StackState<'config>
    for OasisStackState<'ctx, 'backend, 'config, C, Cfg>
{
    fn metadata(&self) -> &StackSubstateMetadata<'config> {
        self.substate.metadata()
    }

    fn metadata_mut(&mut self) -> &mut StackSubstateMetadata<'config> {
        self.substate.metadata_mut()
    }

    fn enter(&mut self, gas_limit: u64, is_static: bool) {
        self.substate.enter(gas_limit, is_static);

        CurrentState::start_transaction();
    }

    fn exit_commit(&mut self) -> Result<(), ExitError> {
        self.substate.exit_commit()?;

        CurrentState::commit_transaction();

        Ok(())
    }

    fn exit_revert(&mut self) -> Result<(), ExitError> {
        self.substate.exit_revert()?;

        CurrentState::rollback_transaction();

        Ok(())
    }

    fn exit_discard(&mut self) -> Result<(), ExitError> {
        self.substate.exit_discard()?;

        CurrentState::rollback_transaction();

        Ok(())
    }

    fn is_empty(&self, address: H160) -> bool {
        self.basic(address).balance == U256::zero()
            && self.basic(address).nonce == U256::zero()
            && self.code(address).len() == 0
    }

    fn deleted(&self, address: H160) -> bool {
        self.substate.deleted(address)
    }

    fn is_cold(&self, address: H160) -> bool {
        self.substate
            .recursive_is_cold(&|a| a.accessed_addresses.contains(&address))
    }

    fn is_storage_cold(&self, address: H160, key: H256) -> bool {
        self.substate
            .recursive_is_cold(&|a: &Accessed| a.accessed_storage.contains(&(address, key)))
    }

    fn inc_nonce(&mut self, address: H160) -> Result<(), ExitError> {
        // Do not increment the origin nonce as that has already been handled by the SDK. But do
        // record that the nonce should be incremented based on EVM semantics so we can adjust any
        // results from the `basic` method.
        if address == self.origin() && !CurrentState::with_env(|env| env.is_simulation()) {
            self.substate.origin_nonce_incremented = true;
            return Ok(());
        }

        let address = Cfg::map_address(address);
        <C::Runtime as Runtime>::Accounts::inc_nonce(address);
        Ok(())
    }

    fn set_storage(&mut self, address: H160, key: H256, value: H256) {
        let address: types::H160 = address.into();
        let key: types::H256 = key.into();
        let value: types::H256 = value.into();

        // We cache the current value if this is the first time we modify it in the transaction.
        if let Entry::Vacant(e) = self.original_storage.entry((address, key)) {
            let original =
                state::with_storage::<Cfg, _, _, _>(self.backend.ctx, &address, |store| {
                    store.get(key).unwrap_or_default()
                });
            // No need to cache if same value.
            if original != value {
                e.insert(original);
            }
        }

        if value == types::H256::default() {
            state::with_storage::<Cfg, _, _, _>(self.backend.ctx, &address, |store| {
                store.remove(key);
            });
        } else {
            state::with_storage::<Cfg, _, _, _>(self.backend.ctx, &address, |store| {
                store.insert(key, value);
            });
        }
    }

    fn reset_storage(&mut self, _address: H160) {
        // Reset storage is ignored since storage cannot be efficiently reset as this would require
        // iterating over all of the storage keys. This is fine as reset_storage is only ever called
        // on non-empty storage when doing SELFDESTRUCT, which we don't support.
    }

    fn log(&mut self, address: H160, topics: Vec<H256>, data: Vec<u8>) {
        self.substate.log(address, topics, data);
    }

    fn set_deleted(&mut self, address: H160) {
        // Note that we will abort during apply if SELFDESTRUCT was used.
        self.substate.set_deleted(address)
    }

    fn set_code(&mut self, address: H160, code: Vec<u8>) {
        CurrentState::with_store(|store| {
            let mut store = state::codes(store);
            store.insert(address, code);
        });
    }

    fn transfer(&mut self, transfer: Transfer) -> Result<(), ExitError> {
        let from = Cfg::map_address(transfer.source);
        let to = Cfg::map_address(transfer.target);
        let amount = transfer.value.as_u128();
        let amount = token::BaseUnits::new(amount, Cfg::TOKEN_DENOMINATION);

        <C::Runtime as Runtime>::Accounts::transfer(from, to, &amount)
            .map_err(|_| ExitError::OutOfFund)
    }

    fn reset_balance(&mut self, _address: H160) {
        // Reset balance is ignored since it exists due to a bug in SELFDESTRUCT, which we
        // don't support.
    }

    fn touch(&mut self, _address: H160) {
        // Do not do anything.
    }
}
