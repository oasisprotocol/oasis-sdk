use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};

use crate::{
    services::kms::{GenerateRequest, GenerateResponse},
    state::AppState,
};

/// Key generation endpoint.
pub async fn generate(
    State(state): State<Arc<AppState>>,
    Json(body): Json<GenerateRequest>,
) -> Result<Json<GenerateResponse>, (StatusCode, String)> {
    state
        .kms
        .generate(&body)
        .await
        .map(Json)
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))
}
