use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};

use crate::{services::kms::OpenSecretRequest, state::AppState};

/// Secret retrieval request.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct SecretGetRequest {
    /// Public secret name.
    pub name: String,
}

/// Secret retrieval response.
#[derive(Clone, Default, serde::Serialize)]
pub struct SecretGetResponse {
    /// Secret name.
    pub name: String,
    /// Secret value
    pub value: String,
}

/// Secret retrieval endpoint.
pub async fn get(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SecretGetRequest>,
) -> Result<Json<SecretGetResponse>, (StatusCode, String)> {
    let secret = state
        .env
        .app_cfg()
        .await
        .map(|app_cfg| app_cfg.secrets)
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?
        .get(&body.name)
        .cloned()
        .ok_or((StatusCode::NOT_FOUND, "secret not found".to_string()))?;

    let secret = state
        .kms
        .open_secret(&OpenSecretRequest {
            name: &body.name,
            value: &secret,
            context: None,
        })
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    let name = String::from_utf8(secret.name)
        .map_err(|err| (StatusCode::UNSUPPORTED_MEDIA_TYPE, err.to_string()))?;
    let value = String::from_utf8(secret.value)
        .map_err(|err| (StatusCode::UNSUPPORTED_MEDIA_TYPE, err.to_string()))?;

    Ok(Json(SecretGetResponse { name, value }))
}
