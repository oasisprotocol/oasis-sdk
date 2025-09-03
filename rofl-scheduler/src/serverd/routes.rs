use std::sync::Arc;

use axum::{extract, response};

use oasis_runtime_sdk::{
    core::host::{bundle_manager, log_manager},
    types::address::Address,
};
use oasis_runtime_sdk_rofl_market::types::InstanceId;

use crate::types::ACTION_LOG_VIEW;

use super::{auth::Claims, error::Error, State};

/// Request to get logs.
#[derive(Debug, Default, Clone, serde::Deserialize)]
pub struct LogsGetRequest {
    pub instance_id: String,
    #[serde(default)]
    pub component_id: String,
    #[serde(default)]
    pub since: u64,
}

/// Response from the fetch logs endpoint.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct LogsGetResponse {
    pub logs: Vec<String>,
}

/// Fetch logs endpoint.
pub async fn logs_get(
    claims: Claims,
    extract::State(state): extract::State<Arc<State>>,
    extract::Json(mut request): extract::Json<LogsGetRequest>,
) -> Result<response::Json<LogsGetResponse>, Error> {
    let instance_id: InstanceId = request.instance_id.parse()?;

    // Ensure instance is owned by the authenticated user.
    let instance = state
        .manager
        .get_instance(&instance_id)
        .ok_or(Error::NotFound)?;
    let address = Address::from_bech32(&claims.address)?;
    if !instance.has_permission(ACTION_LOG_VIEW, address) {
        return Err(Error::Forbidden);
    }

    // When component identifier is not passed, use the first component.
    let labels = crate::manager::labels_for_instance(instance_id);
    if request.component_id.is_empty() {
        let response = state
            .env
            .host()
            .bundle_manager()
            .bundle_list(bundle_manager::BundleListRequest {
                labels: labels.clone(),
            })
            .await?;
        if response.bundles.is_empty() || response.bundles[0].components.is_empty() {
            return Err(Error::NotFound);
        }
        request.component_id = format!("rofl.{}", response.bundles[0].components[0].name);
    }

    let response = state
        .env
        .host()
        .log_manager()
        .log_get(log_manager::LogGetRequest {
            labels,
            component_id: request.component_id,
            since: request.since,
        })
        .await?;

    Ok(response::Json(LogsGetResponse {
        logs: response.logs,
    }))
}
