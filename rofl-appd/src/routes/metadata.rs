use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use axum::{extract::State, http::StatusCode, Json};

use crate::state::AppState;

/// Set metadata endpoint.
pub async fn set(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BTreeMap<String, String>>,
) -> Result<(), (StatusCode, String)> {
    state
        .metadata
        .set(body)
        .await
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))
}

/// Upsert metadata endpoint.
///
/// Inserts or updates the given key-value pairs, leaving other keys untouched.
pub async fn upsert(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BTreeMap<String, String>>,
) -> Result<(), (StatusCode, String)> {
    state
        .metadata
        .upsert(body)
        .await
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))
}

/// Delete metadata endpoint.
///
/// Removes the given keys, ignoring those that do not exist.
pub async fn delete(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BTreeSet<String>>,
) -> Result<(), (StatusCode, String)> {
    state
        .metadata
        .delete(body)
        .await
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))
}

/// Get metadata endpoint.
pub async fn get(
    State(state): State<Arc<AppState>>,
) -> Result<Json<BTreeMap<String, String>>, (StatusCode, String)> {
    state
        .metadata
        .get()
        .await
        .map(Json)
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}
