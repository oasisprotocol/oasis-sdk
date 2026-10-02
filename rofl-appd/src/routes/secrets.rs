use std::sync::Arc;

use rocket::{http::Status, serde::json::Json, State};

use crate::{
    services::kms::{KmsService, OpenSecretRequest},
    state::Env,
};

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
#[rocket::post("/get", data = "<body>")]
pub async fn get(
    body: Json<SecretGetRequest>,
    env: &State<Arc<dyn Env>>,
    kms: &State<Arc<dyn KmsService>>,
) -> Result<Json<SecretGetResponse>, (Status, String)> {
    let secret = env
        .app_cfg()
        .await
        .map(|app_cfg| Json(app_cfg.secrets))
        .map_err(|err| (Status::InternalServerError, err.to_string()))?
        .get(&body.name)
        .cloned()
        .ok_or((Status::NotFound, "secret not found".to_string()))?;

    let secret = kms
        .open_secret(&OpenSecretRequest {
            name: &body.name,
            value: &secret,
            context: None,
        })
        .await
        .map_err(|err| (Status::InternalServerError, err.to_string()))?;

    let name = String::from_utf8(secret.name)
        .map_err(|err| (Status::UnsupportedMediaType, err.to_string()))?;
    let value = String::from_utf8(secret.value)
        .map_err(|err| (Status::UnsupportedMediaType, err.to_string()))?;

    Ok(Json(SecretGetResponse { name, value }))
}
