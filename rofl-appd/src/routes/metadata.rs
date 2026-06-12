use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use rocket::{http::Status, serde::json::Json, suppress, State};

use crate::services::metadata::MetadataService;

/// Set metadata endpoint.
#[rocket::post("/", data = "<body>")]
pub async fn set(
    body: Json<BTreeMap<String, String>>,
    metadata: &State<Arc<dyn MetadataService>>,
) -> Result<(), (Status, String)> {
    metadata
        .set(body.into_inner())
        .await
        .map_err(|err| (Status::BadRequest, err.to_string()))
}

/// Upsert metadata endpoint.
///
/// Inserts or updates the given key-value pairs, leaving other keys untouched.
#[rocket::put("/", data = "<body>")]
pub async fn upsert(
    body: Json<BTreeMap<String, String>>,
    metadata: &State<Arc<dyn MetadataService>>,
) -> Result<(), (Status, String)> {
    metadata
        .upsert(body.into_inner())
        .await
        .map_err(|err| (Status::BadRequest, err.to_string()))
}

/// Delete metadata endpoint.
///
/// Removes the given keys, ignoring those that do not exist.
// Rocket complains, if DELETE requires body. Suppress it.
#[suppress(dubious_payload)]
#[rocket::delete("/", data = "<body>")]
pub async fn delete(
    body: Json<BTreeSet<String>>,
    metadata: &State<Arc<dyn MetadataService>>,
) -> Result<(), (Status, String)> {
    metadata
        .delete(body.into_inner())
        .await
        .map_err(|err| (Status::BadRequest, err.to_string()))
}

/// Get metadata endpoint.
#[rocket::get("/")]
pub async fn get(
    metadata: &State<Arc<dyn MetadataService>>,
) -> Result<Json<BTreeMap<String, String>>, (Status, String)> {
    metadata
        .get()
        .await
        .map(Json)
        .map_err(|err| (Status::InternalServerError, err.to_string()))
}
