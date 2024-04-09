use crate::{
    core::{
        common::crypto::{hash::Hash, signature::PublicKey as CorePublicKey},
        consensus::beacon::EpochTime,
    },
    crypto::signature::PublicKey,
    state::CurrentState,
    storage::{self, Store},
};

use super::{types, Error, MODULE_NAME};

/// Map of H(RAK)s to their registrations.
const REGISTRATIONS: &[u8] = &[0x01];
/// Map of H(pk)s to KeyEndorsementInfos. This is used when just the public key is needed to avoid
/// fetching entire registrations from storage.
const ENDORSERS: &[u8] = &[0x02];
/// A queue of registration expirations.
const EXPIRATION_QUEUE: &[u8] = &[0x03];

/// Information about an endorsed key.
#[derive(Clone, Debug, Default, PartialEq, Eq, cbor::Encode, cbor::Decode)]
#[cbor(as_array)]
pub struct KeyEndorsementInfo {
    /// Identifier of node that endorsed the enclave.
    pub node_id: CorePublicKey,
    /// RAK of the enclave that endorsed the key. This is only set for endorsements of extra keys.
    pub rak: Option<CorePublicKey>,
}

impl KeyEndorsementInfo {
    /// Create a new key endorsement information for RAK endorsed by given node directly.
    pub fn for_rak(node_id: CorePublicKey) -> Self {
        Self {
            node_id,
            ..Default::default()
        }
    }

    /// Create a new key endorsement information for extra key endorsed by RAK.
    pub fn for_extra_key(node_id: CorePublicKey, rak: CorePublicKey) -> Self {
        Self {
            node_id,
            rak: Some(rak),
        }
    }
}

/// Updates registration of the given ROFL enclave.
pub fn update_registration(registration: types::Registration) -> Result<(), Error> {
    let hrak = hash_rak(&registration.rak);

    // Update expiration queue.
    if let Some(existing) = get_registration(&registration.rak) {
        // Disallow modification of extra keys.
        if existing.extra_keys != registration.extra_keys {
            return Err(Error::ExtraKeyUpdateNotAllowed);
        }

        remove_expiration_queue(existing.expiration, hrak);
    }
    insert_expiration_queue(registration.expiration, hrak, registration.rak);

    // Update registration.
    CurrentState::with_store(|mut root_store| {
        let store = storage::PrefixStore::new(&mut root_store, &MODULE_NAME);
        let mut endorsers = storage::TypedStore::new(storage::PrefixStore::new(store, &ENDORSERS));
        endorsers.insert(hrak, KeyEndorsementInfo::for_rak(registration.node_id));

        for pk in &registration.extra_keys {
            endorsers.insert(
                hash_pk(pk),
                KeyEndorsementInfo::for_extra_key(registration.node_id, registration.rak),
            );
        }

        let store = storage::PrefixStore::new(&mut root_store, &MODULE_NAME);
        let mut registrations =
            storage::TypedStore::new(storage::PrefixStore::new(store, &REGISTRATIONS));
        registrations.insert(hrak, registration);
    });

    Ok(())
}

/// Removes an existing registration of the given ROFL enclave.
pub fn remove_registration(rak: &CorePublicKey) {
    let registration = match get_registration(rak) {
        Some(registration) => registration,
        None => return,
    };

    let hrak = hash_rak(rak);
    // Remove from expiration queue if present.
    remove_expiration_queue(registration.expiration, hrak);

    // Update registration.
    CurrentState::with_store(|mut root_store| {
        let store = storage::PrefixStore::new(&mut root_store, &MODULE_NAME);
        let mut endorsers = storage::TypedStore::new(storage::PrefixStore::new(store, &ENDORSERS));
        endorsers.remove(hrak);

        for pk in &registration.extra_keys {
            endorsers.remove(hash_pk(pk));
        }

        let store = storage::PrefixStore::new(&mut root_store, &MODULE_NAME);
        let mut registrations =
            storage::TypedStore::new(storage::PrefixStore::new(store, &REGISTRATIONS));
        registrations.remove(hrak);
    });
}

/// Retrieves registration of the given ROFL enclave. In case enclave is not registered, returns
/// `None`.
pub fn get_registration(rak: &CorePublicKey) -> Option<types::Registration> {
    let hrak = hash_rak(rak);

    CurrentState::with_store(|store| {
        let store = storage::PrefixStore::new(store, &MODULE_NAME);
        let registrations =
            storage::TypedStore::new(storage::PrefixStore::new(store, &REGISTRATIONS));
        registrations.get(hrak)
    })
}

/// Retrieves endorser of the given ROFL enclave. In case enclave is not registered, returns `None`.
pub fn get_endorser(pk: &PublicKey) -> Option<KeyEndorsementInfo> {
    let hpk = hash_pk(pk);

    CurrentState::with_store(|store| {
        let store = storage::PrefixStore::new(store, &MODULE_NAME);
        let endorsers = storage::TypedStore::new(storage::PrefixStore::new(store, &ENDORSERS));
        endorsers.get(hpk)
    })
}

fn hash_rak(rak: &CorePublicKey) -> Hash {
    hash_pk(&PublicKey::Ed25519(rak.into()))
}

fn hash_pk(pk: &PublicKey) -> Hash {
    Hash::digest_bytes_list(&[pk.key_type().as_bytes(), pk.as_ref()])
}

fn queue_entry_key(epoch: EpochTime, hrak: Hash) -> Vec<u8> {
    [&epoch.to_be_bytes(), hrak.as_ref()].concat()
}

fn insert_expiration_queue(epoch: EpochTime, hrak: Hash, rak: CorePublicKey) {
    CurrentState::with_store(|store| {
        let store = storage::PrefixStore::new(store, &MODULE_NAME);
        let mut queue =
            storage::TypedStore::new(storage::PrefixStore::new(store, &EXPIRATION_QUEUE));
        queue.insert(&queue_entry_key(epoch, hrak), rak);
    })
}

fn remove_expiration_queue(epoch: EpochTime, hrak: Hash) {
    CurrentState::with_store(|store| {
        let store = storage::PrefixStore::new(store, &MODULE_NAME);
        let mut queue = storage::PrefixStore::new(store, &EXPIRATION_QUEUE);
        queue.remove(&queue_entry_key(epoch, hrak));
    })
}

struct ExpirationQueueEntry {
    epoch: EpochTime,
    // hrak is currently not used, so it is not decoded.
}

impl<'a> TryFrom<&'a [u8]> for ExpirationQueueEntry {
    type Error = anyhow::Error;

    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        // Decode a storage key of the format (epoch, hrak).
        if value.len() != Hash::len() + 8 {
            anyhow::bail!("incorrect expiration queue key size");
        }

        Ok(Self {
            epoch: EpochTime::from_be_bytes(value[..8].try_into()?),
            // hrak is currently not used, so it is not decoded.
        })
    }
}

/// Removes all expired registrations, e.g. those that expire in epochs earlier than or equal to the
/// passed epoch.
pub fn expire_registrations(epoch: EpochTime) {
    let expired: Vec<_> = CurrentState::with_store(|store| {
        let store = storage::PrefixStore::new(store, &MODULE_NAME);
        let queue = storage::TypedStore::new(storage::PrefixStore::new(store, &EXPIRATION_QUEUE));

        queue
            .iter()
            .take_while(|(e, _): &(ExpirationQueueEntry, CorePublicKey)| e.epoch <= epoch)
            .map(|(_, rak)| rak)
            .collect()
    });

    for rak in expired {
        remove_registration(&rak);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::testing::{keys, mock};

    #[test]
    fn test_registration() {
        let _mock = mock::Mock::default();
        let rak = keys::alice::pk().try_into().unwrap(); // Fake RAK.
        let rak_pk = keys::alice::pk();

        let registration = get_registration(&rak);
        assert!(registration.is_none());
        let endorser = get_endorser(&rak_pk);
        assert!(endorser.is_none());
        let endorser = get_endorser(&keys::dave::pk());
        assert!(endorser.is_none());

        let new_registration = types::Registration {
            rak,
            expiration: 42,
            extra_keys: vec![
                keys::dave::pk(), // Add dave as an extra endorsed key.
            ],
            ..Default::default()
        };
        update_registration(new_registration.clone()).expect("registration update should work");

        // Ensure extra endorsed keys cannot be updated later.
        let bad_registration = types::Registration {
            extra_keys: vec![],
            ..new_registration.clone()
        };
        update_registration(bad_registration.clone())
            .expect_err("extra endorsed key update should not be allowed");

        let registration = get_registration(&rak).expect("registration should be present");
        assert_eq!(registration, new_registration);
        let endorser = get_endorser(&rak_pk).expect("endorser should be present");
        assert_eq!(endorser.node_id, new_registration.node_id);
        assert!(endorser.rak.is_none());
        let endorser = get_endorser(&keys::dave::pk()).expect("extra keys should be endorsed");
        assert_eq!(endorser.node_id, new_registration.node_id);
        assert_eq!(endorser.rak, Some(rak));

        expire_registrations(42);

        let registration = get_registration(&rak);
        assert!(registration.is_none());
        let endorser = get_endorser(&rak_pk);
        assert!(endorser.is_none());
        let endorser = get_endorser(&keys::dave::pk());
        assert!(endorser.is_none());
    }
}
