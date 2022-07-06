//! Contract storage.
use oasis_contract_sdk_types::storage::StoreKind;
use oasis_runtime_sdk::{
    context::Context,
    dispatcher,
    keymanager::{self, StateKey},
    storage::{self, Store},
};

use crate::{results, state, types, Error, MODULE_NAME};

/// Confidential store key pair ID domain separation context base.
pub const CONFIDENTIAL_STORE_KEY_PAIR_ID_CONTEXT_BASE: &[u8] =
    b"oasis-runtime-sdk/contracts: state";

const CONTEXT_KEY_CONFIDENTIAL_STORE_INSTANCE_COUNT: &str = "contracts.ConfidentialStoreCounter";

/// Create a contract instance store.
///
/// Confidential stores will only work when private key queries for the key
/// manager are available. In others, an error will be returned describing the
/// particular key manager failure.
pub fn for_instance<'a, C: Context>(
    ctx: &'a mut C,
    instance_info: &types::Instance,
    store_kind: StoreKind,
) -> Result<Box<dyn Store + 'a>, Error> {
    // subcall_count, instance_count, round are all used as nonce derivation context
    // in the confidential store. Along with confidential_key, they all need ctx,
    // which becomes unavailable after the first PrefixStore is created, since that
    // keeps a mutable reference to it (via runtime_state()).
    let subcall_count = if let StoreKind::Confidential = store_kind {
        results::get_current_subcall_depth(ctx)
    } else {
        0
    };
    let instance_count: Option<usize> = if let StoreKind::Confidential = store_kind {
        let cnt = *ctx
            .value(CONTEXT_KEY_CONFIDENTIAL_STORE_INSTANCE_COUNT)
            .or_default();
        ctx.value(CONTEXT_KEY_CONFIDENTIAL_STORE_INSTANCE_COUNT)
            .set(cnt + 1);
        Some(cnt)
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

    let contract_state = get_instance_raw_store(ctx, instance_info, store_kind);

    match store_kind {
        // For public storage we use a hashed store using the Blake3 hash function.
        StoreKind::Public => Ok(Box::new(storage::HashedStore::<_, blake3::Hasher>::new(
            contract_state,
        ))),

        StoreKind::Confidential => {
            let confidential_store = storage::ConfidentialStore::new_with_key(
                contract_state,
                confidential_key.unwrap().0,
                &[
                    round.to_le_bytes().as_slice(),
                    subcall_count.to_le_bytes().as_slice(),
                    instance_count.unwrap().to_le_bytes().as_slice(),
                ],
            );
            Ok(Box::new(confidential_store))
        }
    }
}

/// Return the public of confidential raw store of the provided contract instance.
pub fn get_instance_raw_store<'a, C: Context>(
    ctx: &'a mut C,
    instance_info: &types::Instance,
    store_kind: StoreKind,
) -> impl Store + 'a {
    let store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
    let instance_prefix = instance_info.id.to_storage_key();
    let contract_state = storage::PrefixStore::new(
        storage::PrefixStore::new(store, &state::INSTANCE_STATE),
        instance_prefix,
    );

    storage::PrefixStore::new(contract_state, store_kind.prefix())
}
