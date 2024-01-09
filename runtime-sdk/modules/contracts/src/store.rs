//! Contract storage.
use oasis_contract_sdk_types::storage::StoreKind;
use oasis_runtime_sdk::{
    context::Context,
    dispatcher,
    keymanager::{self, StateKey},
    state::CurrentState,
    storage::{self, Store},
    subcall,
};

use crate::{state, types, Error, MODULE_NAME};

/// Confidential store key pair ID domain separation context base.
pub const CONFIDENTIAL_STORE_KEY_PAIR_ID_CONTEXT_BASE: &[u8] =
    b"oasis-runtime-sdk/contracts: state";

const CONTEXT_KEY_CONFIDENTIAL_STORE_INSTANCE_COUNT: &str = "contracts.ConfidentialStoreCounter";

/// Run a closure with the contract instance store.
///
/// Confidential stores will only work when private key queries for the key
/// manager are available. In others, an error will be returned describing the
/// particular key manager failure.
pub fn with_instance_store<C, F, R>(
    ctx: &C,
    instance_info: &types::Instance,
    store_kind: StoreKind,
    f: F,
) -> Result<R, Error>
where
    C: Context,
    F: FnOnce(&mut dyn Store) -> R,
{
    // subcall_count, instance_count, round are all used as nonce derivation context
    // in the confidential store. Along with confidential_key, they all need ctx,
    // which becomes unavailable after the first PrefixStore is created, since that
    // keeps a mutable reference to it (via runtime_state()).
    let subcall_count = if let StoreKind::Confidential = store_kind {
        subcall::get_current_subcall_depth(ctx)
    } else {
        0
    };
    let instance_count: Option<usize> = if let StoreKind::Confidential = store_kind {
        CurrentState::with(|state| {
            let cnt = *state
                .block_value(CONTEXT_KEY_CONFIDENTIAL_STORE_INSTANCE_COUNT)
                .or_default();
            state
                .block_value(CONTEXT_KEY_CONFIDENTIAL_STORE_INSTANCE_COUNT)
                .set(cnt + 1);
            Some(cnt)
        })
    } else {
        None
    };
    let round = ctx.runtime_header().round;
    let confidential_key: Option<StateKey> = if let StoreKind::Confidential = store_kind {
        let kmgr_client = ctx.key_manager().ok_or(Error::Unsupported)?;
        let kid = keymanager::get_key_pair_id([
            CONFIDENTIAL_STORE_KEY_PAIR_ID_CONTEXT_BASE,
            &instance_info.id.to_storage_key(),
        ]);
        let kp = kmgr_client
            .get_or_create_keys(kid)
            .map_err(|err| Error::Abort(dispatcher::Error::KeyManagerFailure(err)))?;
        Some(kp.state_key)
    } else {
        None
    };

    with_instance_raw_store(instance_info, store_kind, |contract_state| {
        match store_kind {
            // For public storage we use a hashed store using the Blake3 hash function.
            StoreKind::Public => Ok(f(&mut storage::HashedStore::<_, blake3::Hasher>::new(
                contract_state,
            ))),

            StoreKind::Confidential => {
                let mut confidential_store = storage::ConfidentialStore::new_with_key(
                    contract_state,
                    confidential_key.unwrap().0,
                    &[
                        round.to_le_bytes().as_slice(),
                        subcall_count.to_le_bytes().as_slice(),
                        instance_count.unwrap().to_le_bytes().as_slice(),
                    ],
                );
                Ok(f(&mut confidential_store))
            }
        }
    })
}

/// Run a closure with the per-contract-instance raw (public) store.
pub fn with_instance_raw_store<F, R>(
    instance_info: &types::Instance,
    store_kind: StoreKind,
    f: F,
) -> R
where
    F: FnOnce(&mut dyn Store) -> R,
{
    CurrentState::with_store(|store| {
        let store = storage::PrefixStore::new(store, &MODULE_NAME);
        let instance_prefix = instance_info.id.to_storage_key();
        let contract_state = storage::PrefixStore::new(
            storage::PrefixStore::new(store, &state::INSTANCE_STATE),
            instance_prefix,
        );

        let mut store = storage::PrefixStore::new(contract_state, store_kind.prefix());
        f(&mut store)
    })
}
