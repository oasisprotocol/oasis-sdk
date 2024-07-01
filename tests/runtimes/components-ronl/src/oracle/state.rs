use oasis_runtime_sdk::{state::CurrentState, storage};

use super::{types, MODULE_NAME};

/// Current aggregation round state.
const CURRENT_ROUND: &[u8] = &[0x01];
/// Last observation.
const LAST_OBSERVATION: &[u8] = &[0x02];

/// Retrieves the current aggregation round state.
pub fn get_current_round() -> types::Round {
    CurrentState::with_store(|store| {
        let store = storage::TypedStore::new(storage::PrefixStore::new(store, &MODULE_NAME));
        store.get(CURRENT_ROUND).unwrap_or_default()
    })
}

/// Sets the current aggregation round state.
pub fn set_current_round(round: types::Round) {
    CurrentState::with_store(|store| {
        let mut store = storage::TypedStore::new(storage::PrefixStore::new(store, &MODULE_NAME));
        store.insert(CURRENT_ROUND, round);
    })
}

/// Retrieves the last observation.
pub fn get_last_observation() -> Option<types::Observation> {
    CurrentState::with_store(|store| {
        let store = storage::TypedStore::new(storage::PrefixStore::new(store, &MODULE_NAME));
        store.get(LAST_OBSERVATION)
    })
}

/// Sets the last observation.
pub fn set_last_observation(observation: Option<types::Observation>) {
    CurrentState::with_store(|store| {
        let mut store = storage::TypedStore::new(storage::PrefixStore::new(store, &MODULE_NAME));
        if let Some(observation) = observation {
            store.insert(LAST_OBSERVATION, observation);
        } else {
            store.remove(LAST_OBSERVATION);
        }
    })
}
