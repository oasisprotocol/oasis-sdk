use std::sync::Arc;

use rocket::{http::Status, serde::json::Json, State};
use serde_with::serde_as;

use crate::state::Env;

/// Query request.
#[serde_as]
#[derive(Clone, Debug, serde::Deserialize)]
pub struct QueryRequest {
    /// Method name.
    pub method: String,
    /// CBOR encoded arguments.
    #[serde_as(as = "serde_with::hex::Hex")]
    pub args: Vec<u8>,
}

/// Query response.
#[serde_as]
#[derive(Clone, Default, serde::Serialize)]
pub struct QueryResponse {
    /// Raw response data.
    #[serde_as(as = "serde_with::hex::Hex")]
    pub data: Vec<u8>,
}

/// Query submits a query to the registration paratime.
#[rocket::post("/", data = "<body>")]
pub async fn query(
    body: Json<QueryRequest>,
    env: &State<Arc<dyn Env>>,
) -> Result<Json<QueryResponse>, (Status, String)> {
    let result = env
        .query(&body.method, body.args.clone())
        .await
        .map_err(|err| (Status::InternalServerError, format!("Query failed: {err}")))?;

    // Return the response.
    let response = QueryResponse { data: result };

    Ok(Json(response))
}
