//! Settings → CHNG and the CHNG ticket list. Tokens are write-only; see
//! `services::chng`.

use axum::{
    Router,
    extract::{Path, Query, State},
    response::Json as ResponseJson,
    routing::get,
};
use deployment::Deployment;
use serde::Deserialize;
use services::services::chng::{
    self, ChngSettings, ChngTicketList, HashmalChngLookup, UpdateChngSettingsRequest,
};
use utils::response::ApiResponse;

use crate::{DeploymentImpl, error::ApiError};

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/chng/settings", get(get_settings).put(update_settings))
        .route("/chng/tickets", get(list_tickets))
        .route("/chng/tickets/{key}/hashmal", get(hashmal_lookup))
}

async fn get_settings(
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<ChngSettings>>, ApiError> {
    let settings = chng::get_settings(&deployment.db().pool).await?;
    Ok(ResponseJson(ApiResponse::success(settings)))
}

async fn update_settings(
    State(deployment): State<DeploymentImpl>,
    ResponseJson(request): ResponseJson<UpdateChngSettingsRequest>,
) -> Result<ResponseJson<ApiResponse<ChngSettings>>, ApiError> {
    let settings = chng::update_settings(&deployment.db().pool, request).await?;
    Ok(ResponseJson(ApiResponse::success(settings)))
}

async fn list_tickets(
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<ChngTicketList>>, ApiError> {
    let tickets = chng::search_tickets(&deployment.db().pool).await?;
    Ok(ResponseJson(ApiResponse::success(tickets)))
}

#[derive(Debug, Deserialize)]
struct HashmalLookupQuery {
    session_id: Option<String>,
}

async fn hashmal_lookup(
    State(deployment): State<DeploymentImpl>,
    Path(key): Path<String>,
    Query(query): Query<HashmalLookupQuery>,
) -> Result<ResponseJson<ApiResponse<HashmalChngLookup>>, ApiError> {
    if !chng::is_valid_ticket_key(&key) {
        return Err(ApiError::BadRequest(
            "CHNG ticket key must look like CSEQ-123".into(),
        ));
    }
    let session_id = query.session_id.filter(|id| !id.is_empty());
    if session_id
        .as_deref()
        .is_some_and(|id| !chng::is_valid_session_id(id))
    {
        return Err(ApiError::BadRequest("Invalid Hashmal session id".into()));
    }
    let lookup = chng::hashmal_lookup(&deployment.db().pool, &key, session_id.as_deref()).await;
    Ok(ResponseJson(ApiResponse::success(lookup)))
}
