use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ApiResponseEnvelope<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
    /// Typed error payload (`ApiResponse::error_with_data`). Surfaced to the
    /// agent when a route set no `message`, so it never reads "Unknown error".
    #[serde(default)]
    pub error_data: Option<serde_json::Value>,
}

pub mod backend;
pub mod task_server;
