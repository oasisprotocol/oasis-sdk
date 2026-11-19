use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};
use serde_with::serde_as;

use crate::state::AppState;

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
pub async fn query(
    State(state): State<Arc<AppState>>,
    Json(body): Json<QueryRequest>,
) -> Result<Json<QueryResponse>, (StatusCode, String)> {
    let result = state
        .env
        .query(&body.method, body.args)
        .await
        .map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Query failed: {err}"),
            )
        })?;

    // Return the response.
    let response = QueryResponse { data: result };

    Ok(Json(response))
}
