use std::collections::BTreeMap;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use cluster_protocol::{PreviewHttpRequest, PreviewHttpResponse};
use reqwest::{Client, Method};
use thiserror::Error;

const MAX_PREVIEW_BODY_BYTES: usize = 50 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum PreviewError {
    #[error("invalid preview request: {0}")]
    Invalid(String),
    #[error("preview upstream failed: {0}")]
    Upstream(#[from] reqwest::Error),
}

#[derive(Clone)]
pub struct PreviewService {
    client: Client,
}

impl PreviewService {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("preview client must build"),
        }
    }

    pub async fn proxy(
        &self,
        request: PreviewHttpRequest,
    ) -> Result<PreviewHttpResponse, PreviewError> {
        if request.generation == 0 || request.port == 0 || !request.path_and_query.starts_with('/')
        {
            return Err(PreviewError::Invalid(
                "invalid target generation, port, or path".into(),
            ));
        }
        let method = Method::from_bytes(request.method.as_bytes())
            .map_err(|error| PreviewError::Invalid(error.to_string()))?;
        let body = BASE64_STANDARD
            .decode(request.body_base64)
            .map_err(|error| PreviewError::Invalid(error.to_string()))?;
        if body.len() > MAX_PREVIEW_BODY_BYTES {
            return Err(PreviewError::Invalid("preview body exceeds limit".into()));
        }
        let url = format!(
            "http://127.0.0.1:{}{}",
            request.port, request.path_and_query
        );
        let mut upstream = self
            .client
            .request(method, url)
            .header("accept-encoding", "identity");
        for (name, value) in request.headers {
            if !is_hop_by_hop(&name) {
                upstream = upstream.header(name, value);
            }
        }
        let response = upstream.body(body).send().await?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.to_string(), value.to_string()))
            })
            .collect::<BTreeMap<_, _>>();
        let bytes = response.bytes().await?;
        if bytes.len() > MAX_PREVIEW_BODY_BYTES {
            return Err(PreviewError::Invalid(
                "preview response exceeds limit".into(),
            ));
        }
        Ok(PreviewHttpResponse {
            status,
            headers,
            body_base64: BASE64_STANDARD.encode(bytes),
        })
    }
}

impl Default for PreviewService {
    fn default() -> Self {
        Self::new()
    }
}

fn is_hop_by_hop(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection" | "proxy-connection" | "keep-alive" | "transfer-encoding" | "upgrade"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_hop_by_hop_headers() {
        assert!(is_hop_by_hop("Connection"));
        assert!(is_hop_by_hop("upgrade"));
        assert!(!is_hop_by_hop("content-type"));
    }
}
