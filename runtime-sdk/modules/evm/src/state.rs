use oasis_runtime_sdk::{
    context::Context,
    state::CurrentState,
    storage::{ConfidentialStore, HashedStore, PrefixStore, Store, TypedStore},
};

use crate::{types::H160, Config};

/// Prefix for Ethereum account code in our storage (maps H160 -> Vec<u8>).
pub const CODES: &[u8] = &[0x01];
/// Prefix for Ethereum account storage in our storage (maps H160||H256 -> H256).
pub const STORAGES: &[u8] = &[0x02];
/// Prefix for Ethereum block hashes (only for last BLOCK_HASH_WINDOW_SIZE blocks
/// excluding current) storage in our storage (maps Round -> H256).
pub const BLOCK_HASHES: &[u8] = &[0x03];
/// Prefix for Ethereum account storage in our confidential storage (maps H160||H256 -> H256).
pub const CONFIDENTIAL_STORAGES: &[u8] = &[0x04];

/// Confidential store key pair ID domain separation context base.
pub const CONFIDENTIAL_STORE_KEY_PAIR_ID_CONTEXT_BASE: &[u8] = b"oasis-runtime-sdk/evm: state";
const CONTEXT_KEY_CONFIDENTIAL_STORE_INSTANCE_COUNT: &str = "evm.ConfidentialStoreCounter";

/// The number of hash blocks that can be obtained from the current blockchain.
pub const BLOCK_HASH_WINDOW_SIZE: u64 = 256;

/// Run closure with the store of the provided contract address. Based on configuration this will
/// be either confidential or public storage.
pub fn with_storage<Cfg, C, F, R>(ctx: &C, address: &H160, f: F) -> R
where
    Cfg: Config,
    C: Context,
    F: FnOnce(&mut TypedStore<&mut dyn Store>) -> R,
{
    if Cfg::CONFIDENTIAL {
        with_confidential_storage(ctx, address, f)
    } else {
        with_public_storage(address, f)
    }
}

/// Run closure with the public store of the provided contract address.
pub fn with_public_storage<F, R>(address: &H160, f: F) -> R
where
    F: FnOnce(&mut TypedStore<&mut dyn Store>) -> R,
{
    CurrentState::with_store(|store| {
        let mut store =
            HashedStore::<_, blake3::Hasher>::new(contract_storage(store, STORAGES, address));
        let mut store = TypedStore::new(&mut store as &mut dyn Store);
        f(&mut store)
    })
}

/// Run closure with the confidential store of the provided contract address.
pub fn with_confidential_storage<'a, C, F, R>(ctx: &'a C, address: &'a H160, f: F) -> R
where
    C: Context,
    F: FnOnce(&mut TypedStore<&mut dyn Store>) -> R,
{
    let kmgr_client = ctx
        .key_manager()
        .expect("key manager must be available to use confidentiality");
    let key_id = oasis_runtime_sdk::keymanager::get_key_pair_id([
        CONFIDENTIAL_STORE_KEY_PAIR_ID_CONTEXT_BASE,
        address.as_ref(),
    ]);
    let keypair = kmgr_client
        .get_or_create_keys(key_id)
        .expect("unable to retrieve confidential storage keys");
    let confidential_key = keypair.state_key;

    // These values are used to derive the confidential store nonce:
    let round = ctx.runtime_header().round;
    let instance_count: usize = CurrentState::with(|state| {
        // One state is used per tx batch, so the instance count will monotonically increase.
        let cnt = *state
            .block_value(CONTEXT_KEY_CONFIDENTIAL_STORE_INSTANCE_COUNT)
            .or_default();
        state
            .block_value(CONTEXT_KEY_CONFIDENTIAL_STORE_INSTANCE_COUNT)
            .set(cnt + 1);
        cnt
    });

    CurrentState::with(|state| {
        let mode = state.env().mode() as u8;
        let contract_storages = contract_storage(state.store(), CONFIDENTIAL_STORAGES, address);
        let mut confidential_storages = ConfidentialStore::new_with_key(
            contract_storages,
            confidential_key.0,
            &[
                round.to_le_bytes().as_slice(),
                instance_count.to_le_bytes().as_slice(),
                &[mode],
            ],
        );
        let mut store = TypedStore::new(&mut confidential_storages as &mut dyn Store);
        f(&mut store)
    })
}

fn contract_storage<'a, S: Store + 'a>(
    state: S,
    prefix: &'a [u8],
    address: &'a H160,
) -> PrefixStore<impl Store + 'a, &'a H160> {
    let store = PrefixStore::new(state, &crate::MODULE_NAME);
    let storages = PrefixStore::new(store, prefix);
    PrefixStore::new(storages, address)
}

/// Get a typed store for codes of all contracts.
pub fn codes<'a, S: Store + 'a>(state: S) -> TypedStore<impl Store + 'a> {
    let store = PrefixStore::new(state, &crate::MODULE_NAME);
    TypedStore::new(PrefixStore::new(store, &CODES))
}

/// Get a typed store for historic block hashes.
pub fn block_hashes<'a, S: Store + 'a>(state: S) -> TypedStore<impl Store + 'a> {
    let store = PrefixStore::new(state, &crate::MODULE_NAME);
    TypedStore::new(PrefixStore::new(store, &BLOCK_HASHES))
}
