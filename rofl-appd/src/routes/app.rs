use std::{collections::BTreeMap, sync::Arc};

use rocket::{http::Status, serde::json::Json, State};

use crate::state::Env;

#[rocket::get("/id")]
pub fn id(env: &State<Arc<dyn Env>>) -> String {
    env.app_id().to_bech32()
}

#[rocket::get("/config/metadata")]
pub async fn config_metadata(
    env: &State<Arc<dyn Env>>,
) -> Result<Json<BTreeMap<String, String>>, (Status, String)> {
    env.app_cfg()
        .await
        .map(|app_cfg| Json(app_cfg.metadata))
        .map_err(|err| (Status::InternalServerError, err.to_string()))
}
