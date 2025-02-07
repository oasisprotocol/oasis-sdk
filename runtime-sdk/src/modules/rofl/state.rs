use crate::{
    core::{
        common::crypto::{hash::Hash, signature::PublicKey as CorePublicKey},
        consensus::beacon::EpochTime,
    },
    crypto::signature::PublicKey,
    state::CurrentState,
    storage::{self, Store},
};

use super::{app_id::AppId, types, Error, MODULE_NAME};

/// Map of application identifiers to their configs.
const APPS: &[u8] = &[0x01];
/// Map of (application identifier, H(RAK)) tuples to their registrations.
const REGISTRATIONS: &[u8] = &[0x02];
/// Map of H(pk)s to KeyEndorsementInfos. This is used when just the public key is needed to avoid
/// fetching entire registrations from storage.
const ENDORSERS: &[u8] = &[0x03];
/// A queue of registration expirations.
const EXPIRATION_QUEUE: &[u8] = &[0x04];

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

fn apps<S: Store>(store: S) -> storage::TypedStore<impl Store> {
    let store = storage::PrefixStore::new(store, &MODULE_NAME);
    storage::TypedStore::new(storage::PrefixStore::new(store, &APPS))
}

/// Retrieves an application configuration.
pub fn get_app(app_id: AppId) -> Option<types::AppConfig> {
    CurrentState::with_store(|store| apps(store).get(app_id))
}

/// Retrieves all application configurations.
pub fn get_apps() -> Vec<types::AppConfig> {
    CurrentState::with_store(|store| {
        let store = storage::PrefixStore::new(store, &MODULE_NAME);
        let apps = storage::TypedStore::new(storage::PrefixStore::new(store, &APPS));
        apps.iter()
            .map(|(_, cfg): (AppId, types::AppConfig)| cfg)
            .collect()
    })
}

/// Updates an application configuration.
pub fn set_app(cfg: types::AppConfig) {
    CurrentState::with_store(|store| {
        apps(store).insert(cfg.id, cfg);
    })
}

/// Removes an application configuration.
pub fn remove_app(app_id: AppId) {
    CurrentState::with_store(|store| {
        apps(store).remove(app_id);
    })
}

/// Updates registration of the given ROFL enclave.
pub fn update_registration(registration: types::Registration) -> Result<(), Error> {
    let hrak = hash_rak(&registration.rak);

    // Update expiration queue.
    if let Some(existing) = get_registration_hrak(registration.app, hrak) {
        // Disallow modification of extra keys.
        if existing.extra_keys != registration.extra_keys {
            return Err(Error::ExtraKeyUpdateNotAllowed);
        }

        remove_expiration_queue(existing.expiration, registration.app, hrak);
    }
    insert_expiration_queue(registration.expiration, registration.app, hrak);

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

        let app_id = registration.app;
        let store = storage::PrefixStore::new(&mut root_store, &MODULE_NAME);
        let registrations = storage::PrefixStore::new(store, &REGISTRATIONS);
        let mut app = storage::TypedStore::new(storage::PrefixStore::new(registrations, app_id));
        app.insert(hrak, registration);
    });

    Ok(())
}

fn remove_registration_hrak(app_id: AppId, hrak: Hash) {
    let registration = match get_registration_hrak(app_id, hrak) {
        Some(registration) => registration,
        None => return,
    };

    // Remove from expiration queue if present.
    remove_expiration_queue(registration.expiration, registration.app, hrak);

    // Remove registration.
    CurrentState::with_store(|mut root_store| {
        let store = storage::PrefixStore::new(&mut root_store, &MODULE_NAME);
        let mut endorsers = storage::TypedStore::new(storage::PrefixStore::new(store, &ENDORSERS));
        endorsers.remove(hrak);

        for pk in &registration.extra_keys {
            endorsers.remove(hash_pk(pk));
        }

        let store = storage::PrefixStore::new(&mut root_store, &MODULE_NAME);
        let registrations = storage::PrefixStore::new(store, &REGISTRATIONS);
        let mut app = storage::TypedStore::new(storage::PrefixStore::new(registrations, app_id));
        app.remove(hrak);
    });
}

/// Removes an existing registration of the given ROFL enclave.
pub fn remove_registration(app_id: AppId, rak: &CorePublicKey) {
    remove_registration_hrak(app_id, hash_rak(rak))
}

fn get_registration_hrak(app_id: AppId, hrak: Hash) -> Option<types::Registration> {
    CurrentState::with_store(|store| {
        let store = storage::PrefixStore::new(store, &MODULE_NAME);
        let registrations = storage::PrefixStore::new(store, &REGISTRATIONS);
        let app = storage::TypedStore::new(storage::PrefixStore::new(registrations, app_id));
        app.get(hrak)
    })
}

/// Retrieves registration of the given ROFL enclave. In case enclave is not registered, returns
/// `None`.
pub fn get_registration(app_id: AppId, rak: &CorePublicKey) -> Option<types::Registration> {
    get_registration_hrak(app_id, hash_rak(rak))
}

/// Retrieves all registrations for the given ROFL application.
pub fn get_registrations_for_app(app_id: AppId) -> Vec<types::Registration> {
    CurrentState::with_store(|mut root_store| {
        let store = storage::PrefixStore::new(&mut root_store, &MODULE_NAME);
        let registrations = storage::PrefixStore::new(store, &REGISTRATIONS);
        let app = storage::TypedStore::new(storage::PrefixStore::new(registrations, app_id));

        app.iter()
            .map(|(_, registration): (Hash, types::Registration)| registration)
            .collect()
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

fn queue_entry_key(epoch: EpochTime, app_id: AppId, hrak: Hash) -> Vec<u8> {
    [&epoch.to_be_bytes(), app_id.as_ref(), hrak.as_ref()].concat()
}

fn insert_expiration_queue(epoch: EpochTime, app_id: AppId, hrak: Hash) {
    CurrentState::with_store(|store| {
        let store = storage::PrefixStore::new(store, &MODULE_NAME);
        let mut queue = storage::PrefixStore::new(store, &EXPIRATION_QUEUE);
        queue.insert(&queue_entry_key(epoch, app_id, hrak), &[]);
    })
}

fn remove_expiration_queue(epoch: EpochTime, app_id: AppId, hrak: Hash) {
    CurrentState::with_store(|store| {
        let store = storage::PrefixStore::new(store, &MODULE_NAME);
        let mut queue = storage::PrefixStore::new(store, &EXPIRATION_QUEUE);
        queue.remove(&queue_entry_key(epoch, app_id, hrak));
    })
}

struct ExpirationQueueEntry {
    epoch: EpochTime,
    app_id: AppId,
    hrak: Hash,
}

impl<'a> TryFrom<&'a [u8]> for ExpirationQueueEntry {
    type Error = anyhow::Error;

    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        // Decode a storage key of the format (epoch, hrak).
        if value.len() != 8 + AppId::SIZE + Hash::len() {
            anyhow::bail!("incorrect expiration queue key size");
        }

        Ok(Self {
            epoch: EpochTime::from_be_bytes(value[..8].try_into()?),
            app_id: value[8..8 + AppId::SIZE].try_into()?,
            hrak: value[8 + AppId::SIZE..].into(),
        })
    }
}

/// Removes all expired registrations, e.g. those that expire in epochs earlier than or equal to the
/// passed epoch.
pub fn expire_registrations(epoch: EpochTime, limit: usize) {
    let expired: Vec<_> = CurrentState::with_store(|store| {
        let store = storage::PrefixStore::new(store, &MODULE_NAME);
        let queue = storage::TypedStore::new(storage::PrefixStore::new(store, &EXPIRATION_QUEUE));

        queue
            .iter()
            .take_while(|(e, _): &(ExpirationQueueEntry, CorePublicKey)| e.epoch <= epoch)
            .map(|(e, _)| (e.app_id, e.hrak))
            .take(limit)
            .collect()
    });

    for (app_id, hrak) in expired {
        remove_registration_hrak(app_id, hrak);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::testing::{keys, mock};

    #[test]
    fn test_app_cfg() {
        let _mock = mock::Mock::default();

        let app_id = AppId::from_creator_round_index(keys::alice::address(), 0, 0);
        let app = get_app(app_id);
        assert!(app.is_none());

        let cfg = types::AppConfig {
            id: app_id,
            admin: Some(keys::alice::address()),
            ..Default::default()
        };
        set_app(cfg.clone());
        let app = get_app(app_id).expect("application config should be created");
        assert_eq!(app, cfg);

        let cfg = types::AppConfig { admin: None, ..cfg };
        set_app(cfg.clone());
        let app = get_app(app_id).expect("application config should be updated");
        assert_eq!(app, cfg);

        let apps = get_apps();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0], cfg);

        remove_app(app_id);
        let app = get_app(app_id);
        assert!(app.is_none(), "application should have been removed");
        let apps = get_apps();
        assert_eq!(apps.len(), 0);
    }

    #[test]
    fn test_registration() {
        let _mock = mock::Mock::default();
        let app_id = Default::default();
        let rak = keys::alice::pk().try_into().unwrap(); // Fake RAK.
        let rak_pk = keys::alice::pk();

        let registration = get_registration(app_id, &rak);
        assert!(registration.is_none());
        let endorser = get_endorser(&rak_pk);
        assert!(endorser.is_none());
        let endorser = get_endorser(&keys::dave::pk());
        assert!(endorser.is_none());

        let new_registration = types::Registration {
            app: app_id,
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
            app: app_id,
            extra_keys: vec![],
            ..new_registration.clone()
        };
        update_registration(bad_registration.clone())
            .expect_err("extra endorsed key update should not be allowed");

        let registration = get_registration(app_id, &rak).expect("registration should be present");
        assert_eq!(registration, new_registration);
        let endorser = get_endorser(&rak_pk).expect("endorser should be present");
        assert_eq!(endorser.node_id, new_registration.node_id);
        assert!(endorser.rak.is_none());
        let endorser = get_endorser(&keys::dave::pk()).expect("extra keys should be endorsed");
        assert_eq!(endorser.node_id, new_registration.node_id);
        assert_eq!(endorser.rak, Some(rak));
        let registrations = get_registrations_for_app(new_registration.app);
        assert_eq!(registrations.len(), 1);

        expire_registrations(42, 128);

        let registration = get_registration(app_id, &rak);
        assert!(registration.is_none());
        let endorser = get_endorser(&rak_pk);
        assert!(endorser.is_none());
        let endorser = get_endorser(&keys::dave::pk());
        assert!(endorser.is_none());
        let registrations = get_registrations_for_app(new_registration.app);
        assert_eq!(registrations.len(), 0);
    }
}
