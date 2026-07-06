//! Unauthenticated, HMAC-signed blob upload/download routes for the local-disk
//! storage backend. These are public because `<img src>` and the browser upload
//! XHR carry no bearer token — access is authorized by the signature + expiry
//! embedded in the URL (see [`crate::storage::LocalDiskStorage`]).

use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::put,
};
use serde::Deserialize;

use crate::{AppState, storage::StorageError};

/// Hard attachment size cap, matching the confirm-time limit in
/// `routes/attachments.rs` (`MAX_FILE_SIZE`).
const MAX_FILE_SIZE: usize = 20 * 1024 * 1024;
/// Raw-body limit with slack, so oversize uploads reach the handler and get a
/// clean 413 rather than a bare body-limit rejection.
const BODY_LIMIT: usize = MAX_FILE_SIZE + 1024 * 1024;

#[derive(Debug, Deserialize)]
struct SignatureParams {
    op: String,
    exp: i64,
    sig: String,
}

pub(super) fn public_router() -> Router<AppState> {
    Router::new()
        .route("/blobs/{*path}", put(put_blob).get(get_blob))
        // Override axum's 2MB default so 20MB uploads reach the handler.
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
}

async fn put_blob(
    State(state): State<AppState>,
    Path(path): Path<String>,
    Query(params): Query<SignatureParams>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(storage) = state.blob_storage().and_then(|s| s.local_disk()) else {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Attachment storage not available",
        );
    };
    if params.op != "put"
        || storage
            .verify_signature("put", &path, params.exp, &params.sig)
            .is_err()
    {
        return error(StatusCode::FORBIDDEN, "invalid or expired signature");
    }
    if body.len() > MAX_FILE_SIZE {
        return error(StatusCode::PAYLOAD_TOO_LARGE, "file too large");
    }

    // The client sends `x-ms-blob-type: BlockBlob` (Azure vestige); ignore it.
    // Prefer a real content type from the request, else fall back to inference
    // at read time.
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty() && *v != "application/octet-stream");

    match storage.write_blob(&path, content_type, &body).await {
        // Client contract: `uploadToAzure` requires HTTP 201 on success.
        Ok(()) => StatusCode::CREATED.into_response(),
        Err(e) => storage_error(e),
    }
}

async fn get_blob(
    State(state): State<AppState>,
    Path(path): Path<String>,
    Query(params): Query<SignatureParams>,
) -> Response {
    let Some(storage) = state.blob_storage().and_then(|s| s.local_disk()) else {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Attachment storage not available",
        );
    };
    if params.op != "get"
        || storage
            .verify_signature("get", &path, params.exp, &params.sig)
            .is_err()
    {
        return error(StatusCode::FORBIDDEN, "invalid or expired signature");
    }

    match storage.read_blob(&path).await {
        Ok((data, content_type)) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, content_type),
                (header::CACHE_CONTROL, "private, max-age=300".to_string()),
            ],
            data,
        )
            .into_response(),
        Err(e) => storage_error(e),
    }
}

fn error(status: StatusCode, message: &str) -> Response {
    (status, message.to_string()).into_response()
}

fn storage_error(e: StorageError) -> Response {
    match e {
        StorageError::NotFound(_) => error(StatusCode::NOT_FOUND, "not found"),
        StorageError::InvalidPath(_) | StorageError::Signing(_) => {
            error(StatusCode::FORBIDDEN, "forbidden")
        }
        StorageError::Io(msg) => {
            tracing::error!(error = %msg, "blob storage io error");
            error(StatusCode::INTERNAL_SERVER_ERROR, "storage error")
        }
    }
}
