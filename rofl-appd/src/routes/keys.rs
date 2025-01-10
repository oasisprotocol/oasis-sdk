use std::sync::Arc;

use rocket::{http::Status, serde::json::Json, State};

use crate::services::kms::{GenerateRequest, GenerateResponse, KmsService};

/// Key generation endpoint.
#[rocket::post("/generate", data = "<body>")]
pub async fn generate(
    body: Json<GenerateRequest<'_>>,
    kms: &State<Arc<dyn KmsService>>,
) -> Result<Json<GenerateResponse>, (Status, String)> {
    kms.generate(&body)
        .await
        .map(Json)
        .map_err(|err| (Status::BadRequest, err.to_string()))
}
