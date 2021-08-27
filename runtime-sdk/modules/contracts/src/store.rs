//! Contract storage.
use oasis_contract_sdk_types::storage::StoreKind;
use oasis_runtime_sdk::{
    context::Context,
    storage::{self, Store},
};

use crate::{state, types, Error, MODULE_NAME};

/// Create a contract instance store.
pub fn for_instance<'a, C: Context>(
    ctx: &'a mut C,
    instance_info: &types::Instance,
    store_kind: StoreKind,
) -> Result<impl Store + 'a, Error> {
    let store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
    let instance_prefix = instance_info.id.to_storage_key();
    let contract_state = storage::PrefixStore::new(
        storage::PrefixStore::new(store, &state::INSTANCE_STATE),
        instance_prefix,
    );
    let contract_state = storage::PrefixStore::new(contract_state, store_kind.prefix());

    match store_kind {
        // For public storage we use a hashed store using the Blake3 hash function.
        StoreKind::Public => Ok(storage::HashedStore::<_, blake3::Hasher>::new(
            contract_state,
        )),

        StoreKind::Confidential => Err(Error::Unsupported), // Not yet implemented.
    }
}
