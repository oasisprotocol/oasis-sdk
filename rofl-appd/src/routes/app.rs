use std::{collections::BTreeMap, sync::Arc};

use axum::{extract::State, http::StatusCode, Json};

use crate::state::AppState;

pub async fn id(State(state): State<Arc<AppState>>) -> String {
    state.env.app_id().to_bech32()
}

pub async fn config_metadata(
    State(state): State<Arc<AppState>>,
) -> Result<Json<BTreeMap<String, String>>, (StatusCode, String)> {
    state
        .env
        .app_cfg()
        .await
        .map(|app_cfg| Json(app_cfg.metadata))
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to fetch app config metadata".to_string(),
            )
        })
}
