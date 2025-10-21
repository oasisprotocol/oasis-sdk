use std::{collections::BTreeMap, sync::Arc};

use rocket::{http::Status, serde::json::Json, State};

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
