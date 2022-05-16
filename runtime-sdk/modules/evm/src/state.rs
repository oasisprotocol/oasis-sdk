use crate::types::H160;

use oasis_runtime_sdk::{context::Context, storage};

/// Prefix for Ethereum account code in our storage (maps H160 -> Vec<u8>).
pub const CODES: &[u8] = &[0x01];
/// Prefix for Ethereum account storage in our storage (maps H160||H256 -> H256).
pub const STORAGES: &[u8] = &[0x02];
/// Prefix for Ethereum block hashes (only for last BLOCK_HASH_WINDOW_SIZE blocks
/// excluding current) storage in our storage (maps Round -> H256).
pub const BLOCK_HASHES: &[u8] = &[0x03];
#[cfg(feature = "confidential")]
/// Prefix for Ethereum account storage in our confidential storage (maps H160||H256 -> H256).
pub const CONFIDENTIAL_STORAGES: &[u8] = &[0x04];

#[cfg(feature = "confidential")]
/// Confidential store key pair ID domain separation context base.
pub const CONFIDENTIAL_STORE_KEY_PAIR_ID_CONTEXT_BASE: &[u8] = b"oasis-runtime-sdk/evm: state";
#[cfg(feature = "confidential")]
const CONTEXT_KEY_CONFIDENTIAL_STORE_INSTANCE_COUNT: &str = "evm.ConfidentialStoreCounter";

/// The number of hash blocks that can be obtained from the current blockchain.
pub const BLOCK_HASH_WINDOW_SIZE: u64 = 256;

/// Get a typed store for the given address' storage.
pub fn storage<'a, C: Context>(
    ctx: &'a mut C,
    address: &'a H160,
) -> storage::TypedStore<impl storage::Store + 'a> {
    #[cfg(feature = "confidential")]
    {
        confidential_storage(ctx, address)
    }
    #[cfg(not(feature = "confidential"))]
    {
        public_storage(ctx, address)
    }
}

pub fn public_storage<'a, C: Context>(
    ctx: &'a mut C,
    address: &'a H160,
) -> storage::TypedStore<impl storage::Store + 'a> {
    storage::TypedStore::new(storage::HashedStore::<_, blake3::Hasher>::new(
        contract_storage(ctx.runtime_state(), STORAGES, address),
    ))
}

#[cfg(feature = "confidential")]
pub fn confidential_storage<'a, C: Context>(
    ctx: &'a mut C,
    address: &'a H160,
) -> storage::TypedStore<Box<dyn storage::Store + 'a>> {
    fn empty_store() -> storage::TypedStore<Box<dyn storage::Store>> {
        storage::TypedStore::new(Box::new(storage::EmptyStore::new()))
    }
    let kmgr_client = match ctx.key_manager() {
        Some(kmgr_client) => kmgr_client,
        None => return empty_store(),
    };
    let key_id = oasis_runtime_sdk::keymanager::get_key_pair_id(&[
        CONFIDENTIAL_STORE_KEY_PAIR_ID_CONTEXT_BASE,
        address.as_ref(),
    ]);
    let keypair = match kmgr_client.get_or_create_keys(key_id) {
        Ok(keypair) => keypair,
        Err(_) => return empty_store(),
    };
    let confidential_key = keypair.state_key;

    // These values are used to derive the confidential store nonce:
    let round = ctx.runtime_header().round;
    let instance_count: usize = {
        // One Context is used per tx batch, so the instance count will monotonically increase.
        let cnt = *ctx
            .value(CONTEXT_KEY_CONFIDENTIAL_STORE_INSTANCE_COUNT)
            .or_default();
        ctx.value(CONTEXT_KEY_CONFIDENTIAL_STORE_INSTANCE_COUNT)
            .set(cnt + 1);
        cnt
    };
    let is_simulation = ctx.is_simulation();

    let contract_storages = contract_storage(ctx.runtime_state(), CONFIDENTIAL_STORAGES, address);
    let confidential_storages = storage::ConfidentialStore::new_with_key(
        contract_storages,
        confidential_key.0,
        &[
            round.to_le_bytes().as_slice(),
            instance_count.to_le_bytes().as_slice(),
            &[is_simulation as u8],
        ],
    );
    storage::TypedStore::new(Box::new(confidential_storages))
}

fn contract_storage<'a, S: storage::Store + 'a>(
    state: S,
    prefix: &'a [u8],
    address: &'a H160,
) -> storage::PrefixStore<impl storage::Store + 'a, &'a H160> {
    let store = storage::PrefixStore::new(state, &crate::MODULE_NAME);
    let storages = storage::PrefixStore::new(store, prefix);
    storage::PrefixStore::new(storages, address)
}

/// Get a typed store for codes of all contracts.
pub fn codes<'a, S: storage::Store + 'a>(
    state: S,
) -> storage::TypedStore<impl storage::Store + 'a> {
    let store = storage::PrefixStore::new(state, &crate::MODULE_NAME);
    storage::TypedStore::new(storage::PrefixStore::new(store, &CODES))
}

/// Get a typed store for historic block hashes.
pub fn block_hashes<'a, S: storage::Store + 'a>(
    state: S,
) -> storage::TypedStore<impl storage::Store + 'a> {
    let store = storage::PrefixStore::new(state, &crate::MODULE_NAME);
    storage::TypedStore::new(storage::PrefixStore::new(store, &BLOCK_HASHES))
}
