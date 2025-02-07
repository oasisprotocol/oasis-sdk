use oasis_runtime_sdk::{
    core::common::crypto::hash::Hash,
    state::CurrentState,
    storage::{self, Store},
    types::address::Address,
};

use super::{types, MODULE_NAME};

/// Map of hashed provider addresses to their descriptors.
const PROVIDERS: &[u8] = &[0x01];
/// Map of (provider, offer id) to offers.
const OFFERS: &[u8] = &[0x02];
/// Map of (provider, instance id) to instance descriptors.
const INSTANCES: &[u8] = &[0x03];
/// A per-instance queue of pending commands.
const INSTANCE_COMMANDS: &[u8] = &[0x04];

fn providers<S: Store>(store: S) -> storage::TypedStore<impl Store> {
    let store = storage::PrefixStore::new(store, &MODULE_NAME);
    let providers = storage::PrefixStore::new(store, &PROVIDERS);
    storage::TypedStore::new(providers)
}

fn provider_key(address: Address) -> Hash {
    Hash::digest_bytes(address.as_ref())
}

/// Retrieve a specific provider descriptor.
pub fn get_provider(address: Address) -> Option<types::Provider> {
    CurrentState::with_store(|store| providers(store).get(provider_key(address)))
}

/// Retrieve a list of all provider descriptors.
pub fn get_providers() -> Vec<types::Provider> {
    CurrentState::with_store(|store| {
        providers(store)
            .iter()
            .map(|(_, provider): (Hash, types::Provider)| provider.clone())
            .collect()
    })
}

/// Set a given provider descriptor.
pub fn set_provider(provider: types::Provider) {
    CurrentState::with_store(|store| {
        providers(store).insert(provider_key(provider.address), provider);
    })
}

/// Remove a given provider descriptor.
pub fn remove_provider(address: Address) {
    CurrentState::with_store(|store| {
        providers(store).remove(provider_key(address));
    })
}

fn offers<S: Store>(store: S, provider: Address) -> storage::TypedStore<impl Store> {
    let store = storage::PrefixStore::new(store, &MODULE_NAME);
    let providers = storage::PrefixStore::new(store, &OFFERS);
    let offers = storage::PrefixStore::new(providers, provider_key(provider));
    storage::TypedStore::new(offers)
}

/// Retrieve a specific provider's offer.
pub fn get_offer(provider: Address, id: types::OfferId) -> Option<types::Offer> {
    CurrentState::with_store(|store| offers(store, provider).get(id))
}

/// Retrieve a specific provider's offers.
pub fn get_offers(provider: Address) -> Vec<types::Offer> {
    CurrentState::with_store(|store| {
        offers(store, provider)
            .iter()
            .map(|(_, offer): (types::OfferId, types::Offer)| offer.clone())
            .collect()
    })
}

/// Set a specific offer.
pub fn set_offer(provider: Address, offer: types::Offer) {
    CurrentState::with_store(|store| offers(store, provider).insert(offer.id, offer))
}

/// Remove a specific provider's offer.
pub fn remove_offer(provider: Address, id: types::OfferId) {
    CurrentState::with_store(|store| offers(store, provider).remove(id))
}

fn instances<S: Store>(store: S, provider: Address) -> storage::TypedStore<impl Store> {
    let store = storage::PrefixStore::new(store, &MODULE_NAME);
    let providers = storage::PrefixStore::new(store, &INSTANCES);
    let instances = storage::PrefixStore::new(providers, provider_key(provider));
    storage::TypedStore::new(instances)
}

/// Retrieve a specific provider's instance descriptor.
pub fn get_instance(provider: Address, id: types::InstanceId) -> Option<types::Instance> {
    CurrentState::with_store(|store| instances(store, provider).get(id))
}

/// Retrieve a list of all provider's instance descriptors.
pub fn get_instances(provider: Address) -> Vec<types::Instance> {
    CurrentState::with_store(|store| {
        instances(store, provider)
            .iter()
            .map(|(_, instance): (types::InstanceId, types::Instance)| instance)
            .collect()
    })
}

/// Set a specific instance descriptor.
pub fn set_instance(instance: types::Instance) {
    CurrentState::with_store(|store| {
        instances(store, instance.provider).insert(instance.id, instance);
    })
}

/// Remove a specific provider's instance descriptor.
pub fn remove_instance(provider: Address, id: types::InstanceId) {
    CurrentState::with_store(|store| instances(store, provider).remove(id))
}

fn instance_commands<S: Store>(
    store: S,
    provider: Address,
    id: types::InstanceId,
) -> storage::TypedStore<impl Store> {
    let store = storage::PrefixStore::new(store, &MODULE_NAME);
    let providers = storage::PrefixStore::new(store, &INSTANCE_COMMANDS);
    let instances = storage::PrefixStore::new(providers, provider_key(provider));
    let cmds = storage::PrefixStore::new(instances, id);
    storage::TypedStore::new(cmds)
}

/// Retrieve a range of queued instance commands.
pub fn get_instance_commands(
    provider: Address,
    id: types::InstanceId,
    until: types::CommandId,
) -> Vec<types::QueuedCommand> {
    CurrentState::with_store(|store| {
        instance_commands(store, provider, id)
            .iter()
            .map(|(id, cmd): (types::CommandId, Vec<u8>)| types::QueuedCommand { id, cmd })
            .take_while(|qc| qc.id <= until)
            .collect()
    })
}

/// Set a specific instance command.
pub fn set_instance_command(provider: Address, id: types::InstanceId, qc: types::QueuedCommand) {
    CurrentState::with_store(|store| instance_commands(store, provider, id).insert(qc.id, qc.cmd))
}

/// Remove a specific instance command.
pub fn remove_instance_command(
    provider: Address,
    instance: types::InstanceId,
    id: types::CommandId,
) {
    CurrentState::with_store(|store| instance_commands(store, provider, instance).remove(id))
}

#[cfg(test)]
mod test {
    use oasis_runtime_sdk::testing::{keys, mock};

    use super::*;

    #[test]
    fn test_provider() {
        let _mock = mock::Mock::default();

        let provider = get_provider(keys::alice::address());
        assert!(provider.is_none(), "provider should not exist");

        let dsc = types::Provider {
            address: keys::alice::address(),
            ..Default::default()
        };
        set_provider(dsc.clone());

        let provider = get_provider(keys::alice::address());
        assert_eq!(
            provider,
            Some(dsc.clone()),
            "provider should be correctly set"
        );

        let provider = get_provider(keys::bob::address());
        assert!(provider.is_none(), "different provider should not exist");

        let providers = get_providers();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0], dsc);

        remove_provider(keys::alice::address());

        let provider = get_provider(keys::alice::address());
        assert!(provider.is_none(), "provider should be removed");

        let providers = get_providers();
        assert_eq!(providers.len(), 0);
    }

    #[test]
    fn test_offers() {
        let _mock = mock::Mock::default();

        let offer = get_offer(keys::alice::address(), 42.into());
        assert!(offer.is_none(), "offer should not exist");

        let ofr = types::Offer {
            id: 42.into(),
            ..Default::default()
        };
        set_offer(keys::alice::address(), ofr.clone());

        let offer = get_offer(keys::alice::address(), 42.into());
        assert_eq!(offer, Some(ofr.clone()), "offer should be correctly set");

        let offer = get_offer(keys::alice::address(), 43.into());
        assert!(offer.is_none(), "different offer should not exist");

        let offer = get_offer(keys::bob::address(), 42.into());
        assert!(offer.is_none(), "different offer should not exist");

        let offers = get_offers(keys::alice::address());
        assert_eq!(offers.len(), 1);
        assert_eq!(offers[0], ofr);

        let offers = get_offers(keys::bob::address());
        assert_eq!(offers.len(), 0);

        remove_offer(keys::alice::address(), 42.into());

        let offer = get_offer(keys::alice::address(), 42.into());
        assert!(offer.is_none(), "offer should be removed");

        let offers = get_offers(keys::alice::address());
        assert_eq!(offers.len(), 0);
    }

    #[test]
    fn test_instance() {
        let _mock = mock::Mock::default();

        let instance = get_instance(keys::alice::address(), 42.into());
        assert!(instance.is_none(), "instance should not exist");

        let inst = types::Instance {
            provider: keys::alice::address(),
            id: 42.into(),
            ..Default::default()
        };
        set_instance(inst.clone());

        let instance = get_instance(keys::alice::address(), 42.into());
        assert_eq!(
            instance,
            Some(inst.clone()),
            "instance should be correctly set"
        );

        let instances = get_instances(keys::alice::address());
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0], inst);

        let instance = get_instance(keys::alice::address(), 43.into());
        assert!(instance.is_none(), "different instance should not exist");

        let instance = get_instance(keys::bob::address(), 42.into());
        assert!(instance.is_none(), "different instance should not exist");

        let instances = get_instances(keys::bob::address());
        assert_eq!(instances.len(), 0);

        remove_instance(keys::alice::address(), 42.into());

        let instance = get_instance(keys::alice::address(), 42.into());
        assert!(instance.is_none(), "instance should be removed");

        let instances = get_instances(keys::alice::address());
        assert_eq!(instances.len(), 0);
    }

    #[test]
    fn test_instance_command() {
        let _mock = mock::Mock::default();

        let cmds = get_instance_commands(keys::alice::address(), 42.into(), 100.into());
        assert_eq!(cmds.len(), 0, "there should be no instance commands");

        set_instance_command(
            keys::alice::address(),
            42.into(),
            types::QueuedCommand {
                id: 1.into(),
                cmd: vec![42],
            },
        );
        set_instance_command(
            keys::alice::address(),
            42.into(),
            types::QueuedCommand {
                id: 2.into(),
                cmd: vec![43],
            },
        );
        set_instance_command(
            keys::alice::address(),
            42.into(),
            types::QueuedCommand {
                id: 3.into(),
                cmd: vec![44],
            },
        );

        let cmds = get_instance_commands(keys::alice::address(), 42.into(), 100.into());
        assert_eq!(cmds.len(), 3, "there should be instance commands");
        assert_eq!(cmds[0].id, 1.into());
        assert_eq!(cmds[0].cmd, vec![42]);
        assert_eq!(cmds[1].id, 2.into());
        assert_eq!(cmds[1].cmd, vec![43]);
        assert_eq!(cmds[2].id, 3.into());
        assert_eq!(cmds[2].cmd, vec![44]);

        let cmds = get_instance_commands(keys::alice::address(), 42.into(), 1.into());
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].id, 1.into());
        assert_eq!(cmds[0].cmd, vec![42]);

        let cmds = get_instance_commands(keys::alice::address(), 42.into(), 2.into());
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].id, 1.into());
        assert_eq!(cmds[0].cmd, vec![42]);
        assert_eq!(cmds[1].id, 2.into());
        assert_eq!(cmds[1].cmd, vec![43]);

        let cmds = get_instance_commands(keys::alice::address(), 42.into(), 3.into());
        assert_eq!(cmds.len(), 3);
        assert_eq!(cmds[0].id, 1.into());
        assert_eq!(cmds[0].cmd, vec![42]);
        assert_eq!(cmds[1].id, 2.into());
        assert_eq!(cmds[1].cmd, vec![43]);
        assert_eq!(cmds[2].id, 3.into());
        assert_eq!(cmds[2].cmd, vec![44]);

        let cmds = get_instance_commands(keys::alice::address(), 42.into(), 4.into());
        assert_eq!(cmds.len(), 3);

        let cmds = get_instance_commands(keys::alice::address(), 43.into(), 100.into());
        assert_eq!(cmds.len(), 0);

        let cmds = get_instance_commands(keys::bob::address(), 42.into(), 100.into());
        assert_eq!(cmds.len(), 0);

        remove_instance_command(keys::alice::address(), 42.into(), 1.into());

        let cmds = get_instance_commands(keys::alice::address(), 42.into(), 100.into());
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].id, 2.into());
        assert_eq!(cmds[0].cmd, vec![43]);
        assert_eq!(cmds[1].id, 3.into());
        assert_eq!(cmds[1].cmd, vec![44]);
    }
}
