//! Minimal Jira REST client for the sync feature.
//!
//! Uses API v2 resource shapes (rich text as strings — no ADF) against both
//! Jira Cloud and Server/Data Center. Search pagination differs by
//! deployment: Cloud's legacy `/rest/api/2/search` was removed in 2025, so
//! `cloud_basic` uses `/rest/api/2/search/jql` (`nextPageToken`), while
//! `server_pat` uses classic `/rest/api/2/search` (`startAt`/`total`).
//!
//! Error strings must never contain the credential; auth material only ever
//! goes into request headers.

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;

use super::types::JiraAuthMode;

const SEARCH_PAGE_SIZE: u32 = 100;
/// Hard cap on pages per pass so a pathological JQL can't wedge the
/// reconciler (100 pages x 100 issues).
const MAX_SEARCH_PAGES: u32 = 100;

#[derive(Debug, thiserror::Error)]
pub enum JiraClientError {
    #[error("Jira base URL is invalid")]
    InvalidBaseUrl,
    #[error("Jira returned 401 Unauthorized — check credentials")]
    Unauthorized,
    #[error("Jira returned 403 Forbidden — the credential lacks permission")]
    Forbidden,
    #[error("not found")]
    NotFound,
    #[error("JQL error: {0}")]
    Jql(String),
    #[error("Jira returned {status}: {message}")]
    Api { status: u16, message: String },
    #[error("failed to reach Jira: {0}")]
    Transport(String),
}

impl From<reqwest::Error> for JiraClientError {
    fn from(err: reqwest::Error) -> Self {
        // reqwest errors can embed the URL (never the auth header); strip to
        // the display string which is safe.
        JiraClientError::Transport(err.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct JiraIssueData {
    pub id: String,
    pub key: String,
    pub summary: String,
    pub description: Option<String>,
    pub status_name: String,
    /// Jira status category key: "new" | "indeterminate" | "done".
    pub status_category: Option<String>,
    pub updated: Option<DateTime<Utc>>,
}

pub struct JiraClient {
    http: reqwest::Client,
    base_url: String,
    auth_mode: JiraAuthMode,
    email: Option<String>,
    credential: String,
}

#[derive(Debug, Deserialize)]
struct RawSearchResponse {
    #[serde(default)]
    issues: Vec<RawIssue>,
    /// Server/DC classic search only.
    total: Option<i64>,
    /// Cloud `/search/jql` only.
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawIssue {
    id: String,
    key: String,
    #[serde(default)]
    fields: RawFields,
}

#[derive(Debug, Default, Deserialize)]
struct RawFields {
    summary: Option<String>,
    description: Option<String>,
    status: Option<RawStatus>,
    updated: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawStatus {
    name: String,
    #[serde(rename = "statusCategory")]
    status_category: Option<RawStatusCategory>,
}

#[derive(Debug, Deserialize)]
struct RawStatusCategory {
    key: String,
}

#[derive(Debug, Deserialize)]
struct RawTransitionsResponse {
    #[serde(default)]
    transitions: Vec<RawTransition>,
}

#[derive(Debug, Deserialize)]
struct RawTransition {
    id: String,
    to: RawStatus,
}

/// Jira timestamps look like `2026-07-09T12:34:56.789+0000` (no colon in the
/// offset, so not RFC 3339). Try RFC 3339 first, then the Jira format.
fn parse_jira_datetime(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .or_else(|_| DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f%z"))
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

impl From<RawIssue> for JiraIssueData {
    fn from(raw: RawIssue) -> Self {
        JiraIssueData {
            id: raw.id,
            key: raw.key,
            summary: raw.fields.summary.unwrap_or_default(),
            description: raw.fields.description,
            status_name: raw
                .fields
                .status
                .as_ref()
                .map(|s| s.name.clone())
                .unwrap_or_default(),
            status_category: raw
                .fields
                .status
                .and_then(|s| s.status_category)
                .map(|c| c.key),
            updated: raw.fields.updated.as_deref().and_then(parse_jira_datetime),
        }
    }
}

impl JiraClient {
    pub fn new(
        http: reqwest::Client,
        base_url: &str,
        auth_mode: JiraAuthMode,
        email: Option<String>,
        credential: String,
    ) -> Result<Self, JiraClientError> {
        let base_url = base_url.trim().trim_end_matches('/').to_string();
        if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
            return Err(JiraClientError::InvalidBaseUrl);
        }
        Ok(Self {
            http,
            base_url,
            auth_mode,
            email,
            credential,
        })
    }

    pub fn browse_url(&self, issue_key: &str) -> String {
        format!("{}/browse/{}", self.base_url, issue_key)
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let req = self
            .http
            .request(method, format!("{}{}", self.base_url, path));
        match self.auth_mode {
            JiraAuthMode::CloudBasic => req.basic_auth(
                self.email.clone().unwrap_or_default(),
                Some(self.credential.clone()),
            ),
            JiraAuthMode::ServerPat => req.bearer_auth(self.credential.clone()),
        }
    }

    async fn check_status(
        response: reqwest::Response,
        jql_context: bool,
    ) -> Result<reqwest::Response, JiraClientError> {
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        let body = response.text().await.unwrap_or_default();
        let message = extract_jira_error(&body);
        Err(match status.as_u16() {
            401 => JiraClientError::Unauthorized,
            403 => JiraClientError::Forbidden,
            404 => JiraClientError::NotFound,
            400 if jql_context => JiraClientError::Jql(message),
            code => JiraClientError::Api {
                status: code,
                message,
            },
        })
    }

    /// Validate the credential itself (independent of JQL).
    pub async fn myself(&self) -> Result<(), JiraClientError> {
        let response = self
            .request(reqwest::Method::GET, "/rest/api/2/myself")
            .send()
            .await?;
        Self::check_status(response, false).await?;
        Ok(())
    }

    /// Fetch every issue matching `jql` (paginated; capped at
    /// `MAX_SEARCH_PAGES`). Returns the issues and, on Server/DC, the total.
    pub async fn search_all(
        &self,
        jql: &str,
    ) -> Result<(Vec<JiraIssueData>, Option<i64>), JiraClientError> {
        let mut issues = Vec::new();
        let mut total = None;
        let mut next_page_token: Option<String> = None;
        let mut start_at: i64 = 0;

        for _ in 0..MAX_SEARCH_PAGES {
            let page = self
                .search_page(jql, next_page_token.as_deref(), start_at)
                .await?;
            let page_len = page.issues.len() as i64;
            issues.extend(page.issues.into_iter().map(JiraIssueData::from));
            if page.total.is_some() {
                total = page.total;
            }

            match self.auth_mode {
                JiraAuthMode::CloudBasic => match page.next_page_token {
                    Some(token) => next_page_token = Some(token),
                    None => break,
                },
                JiraAuthMode::ServerPat => {
                    start_at += page_len;
                    if page_len == 0 || total.is_some_and(|t| start_at >= t) {
                        break;
                    }
                }
            }
        }

        Ok((issues, total))
    }

    async fn search_page(
        &self,
        jql: &str,
        next_page_token: Option<&str>,
        start_at: i64,
    ) -> Result<RawSearchResponse, JiraClientError> {
        let fields = "summary,description,status,updated";
        let mut req = match self.auth_mode {
            JiraAuthMode::CloudBasic => {
                let mut req = self
                    .request(reqwest::Method::GET, "/rest/api/2/search/jql")
                    .query(&[
                        ("jql", jql),
                        ("fields", fields),
                        ("maxResults", &SEARCH_PAGE_SIZE.to_string()),
                    ]);
                if let Some(token) = next_page_token {
                    req = req.query(&[("nextPageToken", token)]);
                }
                req
            }
            JiraAuthMode::ServerPat => self
                .request(reqwest::Method::GET, "/rest/api/2/search")
                .query(&[
                    ("jql", jql),
                    ("fields", fields),
                    ("maxResults", &SEARCH_PAGE_SIZE.to_string()),
                    ("startAt", &start_at.to_string()),
                ]),
        };
        req = req.header(reqwest::header::ACCEPT, "application/json");
        let response = Self::check_status(req.send().await?, true).await?;
        Ok(response.json::<RawSearchResponse>().await?)
    }

    /// Approximate match count for test-connection. Cloud only; returns
    /// `None` when the endpoint is unavailable.
    pub async fn approximate_count(&self, jql: &str) -> Option<i64> {
        #[derive(Deserialize)]
        struct CountResponse {
            count: i64,
        }
        let response = self
            .request(
                reqwest::Method::POST,
                "/rest/api/3/search/approximate-count",
            )
            .json(&json!({ "jql": jql }))
            .send()
            .await
            .ok()?;
        if !response.status().is_success() {
            return None;
        }
        response.json::<CountResponse>().await.ok().map(|c| c.count)
    }

    /// Fetch one issue by immutable id or key. `Ok(None)` means deleted.
    pub async fn get_issue(
        &self,
        id_or_key: &str,
    ) -> Result<Option<JiraIssueData>, JiraClientError> {
        let response = self
            .request(
                reqwest::Method::GET,
                &format!("/rest/api/2/issue/{id_or_key}"),
            )
            .query(&[("fields", "summary,description,status,updated")])
            .send()
            .await?;
        match Self::check_status(response, false).await {
            Ok(response) => Ok(Some(response.json::<RawIssue>().await?.into())),
            Err(JiraClientError::NotFound) => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// Update summary and/or description.
    pub async fn update_issue_fields(
        &self,
        id_or_key: &str,
        summary: Option<&str>,
        description: Option<Option<&str>>,
    ) -> Result<(), JiraClientError> {
        let mut fields = serde_json::Map::new();
        if let Some(summary) = summary {
            fields.insert("summary".into(), json!(summary));
        }
        if let Some(description) = description {
            fields.insert("description".into(), json!(description));
        }
        if fields.is_empty() {
            return Ok(());
        }
        let response = self
            .request(
                reqwest::Method::PUT,
                &format!("/rest/api/2/issue/{id_or_key}"),
            )
            .json(&json!({ "fields": fields }))
            .send()
            .await?;
        Self::check_status(response, false).await?;
        Ok(())
    }

    /// Transition the issue to the workflow status named `target_status`.
    /// Errors with a recorded message when no available transition leads there.
    pub async fn transition_to_status(
        &self,
        id_or_key: &str,
        target_status: &str,
    ) -> Result<(), JiraClientError> {
        let response = self
            .request(
                reqwest::Method::GET,
                &format!("/rest/api/2/issue/{id_or_key}/transitions"),
            )
            .send()
            .await?;
        let response = Self::check_status(response, false).await?;
        let transitions = response.json::<RawTransitionsResponse>().await?;

        let transition = transitions
            .transitions
            .into_iter()
            .find(|t| t.to.name.eq_ignore_ascii_case(target_status))
            .ok_or_else(|| JiraClientError::Api {
                status: 400,
                message: format!("no available workflow transition to status \"{target_status}\""),
            })?;

        let response = self
            .request(
                reqwest::Method::POST,
                &format!("/rest/api/2/issue/{id_or_key}/transitions"),
            )
            .json(&json!({ "transition": { "id": transition.id } }))
            .send()
            .await?;
        Self::check_status(response, false).await?;
        Ok(())
    }
}

/// Pull the human-readable message out of a Jira error body
/// (`{"errorMessages": [...], "errors": {...}}`), falling back to a trimmed
/// raw body.
fn extract_jira_error(body: &str) -> String {
    #[derive(Deserialize)]
    struct JiraErrorBody {
        #[serde(default, rename = "errorMessages")]
        error_messages: Vec<String>,
        #[serde(default)]
        errors: serde_json::Map<String, serde_json::Value>,
    }
    if let Ok(parsed) = serde_json::from_str::<JiraErrorBody>(body) {
        let mut parts = parsed.error_messages;
        parts.extend(
            parsed
                .errors
                .into_iter()
                .map(|(k, v)| format!("{k}: {}", v.as_str().unwrap_or_default())),
        );
        if !parts.is_empty() {
            return parts.join("; ");
        }
    }
    let trimmed: String = body.chars().take(300).collect();
    if trimmed.is_empty() {
        "no error detail provided".to_string()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_jira_datetime_formats() {
        assert!(parse_jira_datetime("2026-07-09T12:34:56.789+0000").is_some());
        assert!(parse_jira_datetime("2026-07-09T12:34:56.789+00:00").is_some());
        assert!(parse_jira_datetime("2026-07-09T12:34:56Z").is_some());
        assert!(parse_jira_datetime("not a date").is_none());
    }

    #[test]
    fn extracts_error_messages() {
        let body = r#"{"errorMessages":["Field 'foo' does not exist."],"errors":{}}"#;
        assert_eq!(extract_jira_error(body), "Field 'foo' does not exist.");
        assert_eq!(extract_jira_error(""), "no error detail provided");
    }
}
