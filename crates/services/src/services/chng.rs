//! Settings → CHNG and the CHNG ticket list.
//!
//! The operator's change tickets (Jira issue type CHNG) come from a Jira
//! search they configure here. Starting a session from one first asks Hashmal
//! (the operator's ticket dashboard) which earlier Claude session worked the
//! ticket, through Hashmal's read-only `hashmal_chng_sessions` MCP tool.
//!
//! Tokens are host-key envelopes, write-only through the API (only an `op://`
//! reference is ever returned), and bound to the URL they were saved with: a
//! token is never sent anywhere else. Errors never contain a token, a
//! reference, or an upstream body beyond a bounded, curated message.

use std::{fmt, time::Duration};

use api_types::normalize_secret_reference;
use chrono::{DateTime, Utc};
use db::models::chng_settings::ChngSettingsRow;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use thiserror::Error;
use ts_rs::TS;
use utils::assets::chng_settings_key_path;

use super::{
    environment_secrets::EnvironmentSecretError, github_credentials::resolve_reference,
    mcp_gateway_secrets::McpGatewaySecretStore,
};

/// Hashmal's CHNG filter: change tickets the token's account reported.
pub const DEFAULT_JQL: &str =
    "project = CSEQ AND issuetype = CHNG AND reporter = currentUser() ORDER BY created DESC";
pub const HASHMAL_TOOL: &str = "hashmal_chng_sessions";

const MAX_JQL_CHARS: usize = 2000;
const MAX_EMAIL_CHARS: usize = 320;
const MAX_URL_CHARS: usize = 2048;
const MAX_SECRET_BYTES: usize = 4 * 1024;
const JIRA_PAGE_SIZE: u32 = 100;
const JIRA_MAX_PAGES: usize = 2;
const HASHMAL_TRANSCRIPT_TURNS: u32 = 30;
const MAX_TURNS: usize = 50;
const MAX_TURN_CHARS: usize = 4000;
const MAX_SESSIONS: usize = 50;
const MAX_UPSTREAM_MESSAGE: usize = 300;
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const UPSTREAM_TIMEOUT: Duration = Duration::from_secs(15);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Settings → CHNG as shown in the UI. Literal tokens are never returned.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ChngSettings {
    pub jira_base_url: String,
    pub jira_email: String,
    /// The saved search; the default when none is saved.
    pub jql: String,
    pub default_jql: String,
    pub has_jira_token: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub jira_token_reference: Option<String>,
    pub hashmal_mcp_url: String,
    pub has_hashmal_token: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub hashmal_token_reference: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub updated_at: Option<DateTime<Utc>>,
}

/// A whole-form save. For each token: absent keeps the stored one, an empty
/// string clears it, anything else replaces it.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpdateChngSettingsRequest {
    pub jira_base_url: String,
    pub jira_email: String,
    pub jql: String,
    #[serde(default)]
    #[ts(optional)]
    pub jira_token: Option<String>,
    pub hashmal_mcp_url: String,
    #[serde(default)]
    #[ts(optional)]
    pub hashmal_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ChngTicket {
    pub key: String,
    pub summary: String,
    pub status: String,
    /// Jira's status category key: `new`, `indeterminate` or `done`.
    pub status_category: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub priority: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub created: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub updated: Option<DateTime<Utc>>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ChngTicketList {
    /// False until a Jira URL, email and token are saved.
    pub configured: bool,
    pub jql: String,
    pub tickets: Vec<ChngTicket>,
    /// More tickets matched than one list shows.
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum HashmalLookupState {
    NotConfigured,
    Unavailable,
    NotTracked,
    NoSessions,
    Found,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct HashmalSession {
    pub session_id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub last_active: Option<DateTime<Utc>>,
    pub running: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct HashmalTurn {
    pub role: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tool: Option<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct HashmalTranscript {
    pub session_id: String,
    pub total_turns: u32,
    pub turns: Vec<HashmalTurn>,
}

/// What Hashmal knows about one CHNG ticket's sessions. Never an error: every
/// failure is `unavailable` with a message, so Hashmal can't block a session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct HashmalChngLookup {
    pub state: HashmalLookupState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub message: Option<String>,
    /// The first session is the best match: Hashmal's manual link when
    /// `manual`, else the most recently active.
    pub sessions: Vec<HashmalSession>,
    pub manual: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub transcript: Option<HashmalTranscript>,
}

impl HashmalChngLookup {
    fn empty(state: HashmalLookupState, message: Option<String>) -> Self {
        Self {
            state,
            message,
            sessions: Vec::new(),
            manual: false,
            transcript: None,
        }
    }

    fn unavailable(message: impl Into<String>) -> Self {
        Self::empty(HashmalLookupState::Unavailable, Some(message.into()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Service {
    Jira,
    Hashmal,
}

impl Service {
    fn binding(self, url: &str) -> Vec<u8> {
        let field = match self {
            Self::Jira => "jira",
            Self::Hashmal => "hashmal",
        };
        // Bound to the destination too, so an envelope only decrypts for the
        // URL it was saved with.
        format!("vk-chng-settings|{field}|{url}").into_bytes()
    }
}

impl fmt::Display for Service {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Jira => "Jira",
            Self::Hashmal => "Hashmal",
        })
    }
}

/// Where a token came from, so an authentication failure names what to fix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialSource {
    Saved,
    OnePassword,
}

impl fmt::Display for CredentialSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Saved => "the token saved in Settings → CHNG",
            Self::OnePassword => "the 1Password reference in Settings → CHNG",
        })
    }
}

#[derive(Debug, Error)]
pub enum ChngError {
    #[error(
        "{0} URL must be an https:// URL (http:// only for localhost) without credentials, a query or a fragment"
    )]
    InvalidUrl(&'static str),
    #[error("Jira account email must be a single address of at most 320 characters")]
    InvalidEmail,
    #[error("Enter the Jira account email that owns the API token")]
    MissingEmail,
    #[error("The JQL search must be at most 2000 characters with no control characters")]
    InvalidJql,
    #[error("The {0} token must be at most 4 KiB with no control characters")]
    InvalidToken(&'static str),
    #[error("Re-enter the {0} token: a saved token is only sent to the URL it was saved with")]
    ReenterToken(&'static str),
    #[error("CHNG settings changed while saving; reload Settings and try again")]
    Conflict,
    #[error("The CHNG token encryption key is unavailable; check the Vibe Kanban data directory")]
    KeyUnavailable,
    #[error("The saved {0} token cannot be read; re-enter it in Settings → CHNG")]
    Undecryptable(&'static str),
    #[error("{service} token: {source}")]
    Resolve {
        service: &'static str,
        #[source]
        source: EnvironmentSecretError,
    },
    #[error("Jira at {host} rejected {credential} (HTTP {status}); check the email and API token")]
    JiraRejected {
        host: String,
        credential: CredentialSource,
        status: u16,
    },
    #[error("Jira rejected the CHNG search: {0}")]
    JiraSearch(String),
    #[error("Jira at {0} has no /rest/api/3/search/jql endpoint; CHNG needs Jira Cloud")]
    JiraNotCloud(String),
    #[error("Jira rate-limited the CHNG search; try again shortly")]
    JiraRateLimited,
    #[error("Jira at {0} is unreachable or timed out")]
    JiraUnreachable(String),
    #[error("Jira at {host} returned an unexpected response (HTTP {status})")]
    JiraUnexpected { host: String, status: u16 },
    #[error("CHNG settings storage failed")]
    Database(#[from] sqlx::Error),
}

impl ChngError {
    /// Validation problems the user fixes in the form.
    pub fn is_invalid_input(&self) -> bool {
        matches!(
            self,
            Self::InvalidUrl(_)
                | Self::InvalidEmail
                | Self::MissingEmail
                | Self::InvalidJql
                | Self::InvalidToken(_)
                | Self::ReenterToken(_)
                | Self::JiraSearch(_)
        )
    }

    /// Failures of an upstream service or of resolving its credential.
    pub fn is_upstream(&self) -> bool {
        matches!(
            self,
            Self::Undecryptable(_)
                | Self::Resolve { .. }
                | Self::JiraRejected { .. }
                | Self::JiraNotCloud(_)
                | Self::JiraRateLimited
                | Self::JiraUnreachable(_)
                | Self::JiraUnexpected { .. }
        )
    }
}

// ---------------------------------------------------------------- settings

fn load_store() -> Result<McpGatewaySecretStore, ChngError> {
    McpGatewaySecretStore::load_or_generate(&chng_settings_key_path())
        .map_err(|_| ChngError::KeyUnavailable)
}

fn effective_jql(saved: &str) -> String {
    if saved.trim().is_empty() {
        DEFAULT_JQL.to_owned()
    } else {
        saved.to_owned()
    }
}

/// `https://` URLs, or `http://` on loopback (local test doubles). No
/// credentials, query or fragment; a trailing `/` is dropped. Empty means
/// "not set".
fn normalize_url(value: &str, label: &'static str) -> Result<String, ChngError> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(String::new());
    }
    let invalid = || ChngError::InvalidUrl(label);
    if value.len() > MAX_URL_CHARS {
        return Err(invalid());
    }
    let url = url::Url::parse(value).map_err(|_| invalid())?;
    let loopback = matches!(url.host(), Some(url::Host::Domain("localhost")))
        || matches!(url.host(), Some(url::Host::Ipv4(ip)) if ip.is_loopback())
        || matches!(url.host(), Some(url::Host::Ipv6(ip)) if ip.is_loopback());
    let scheme_ok = url.scheme() == "https" || (url.scheme() == "http" && loopback);
    if !scheme_ok
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid());
    }
    Ok(url.as_str().trim_end_matches('/').to_owned())
}

fn normalize_email(value: &str) -> Result<String, ChngError> {
    let value = value.trim();
    if value.chars().count() > MAX_EMAIL_CHARS
        || value.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(ChngError::InvalidEmail);
    }
    Ok(value.to_owned())
}

/// The stored form of a JQL search: empty for the default.
fn normalize_jql(value: &str) -> Result<String, ChngError> {
    let value = value.trim();
    if value.chars().count() > MAX_JQL_CHARS
        || value
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(ChngError::InvalidJql);
    }
    if value == DEFAULT_JQL {
        return Ok(String::new());
    }
    Ok(value.to_owned())
}

/// A 1Password reference loses copied quotes; a literal token is trimmed.
fn normalize_token(value: &str, label: &'static str) -> Result<String, ChngError> {
    let value = normalize_secret_reference(value).unwrap_or_else(|| value.trim().to_owned());
    if value.len() > MAX_SECRET_BYTES || value.chars().any(char::is_control) {
        return Err(ChngError::InvalidToken(label));
    }
    Ok(value)
}

/// The next envelope for one token. Absent keeps the stored envelope, which
/// is only allowed while its URL is unchanged; a cleared URL drops it.
fn next_envelope(
    service: Service,
    label: &'static str,
    current_url: &str,
    current: Option<&String>,
    next_url: &str,
    submitted: Option<&str>,
    store: &mut Option<McpGatewaySecretStore>,
) -> Result<Option<String>, ChngError> {
    if next_url.is_empty() {
        return Ok(None);
    }
    match submitted {
        None => match current {
            Some(_) if current_url != next_url => Err(ChngError::ReenterToken(label)),
            other => Ok(other.cloned()),
        },
        Some(value) if value.trim().is_empty() => Ok(None),
        Some(value) => {
            let value = normalize_token(value, label)?;
            if store.is_none() {
                *store = Some(load_store()?);
            }
            let store = store.as_ref().expect("store loaded above");
            store
                .encrypt(value.as_bytes(), &service.binding(next_url))
                .map(Some)
                .map_err(|_| ChngError::KeyUnavailable)
        }
    }
}

fn apply_update(
    current: Option<&ChngSettingsRow>,
    request: &UpdateChngSettingsRequest,
    store: &mut Option<McpGatewaySecretStore>,
) -> Result<ChngSettingsRow, ChngError> {
    let blank = ChngSettingsRow::default();
    let current = current.unwrap_or(&blank);
    let jira_base_url = normalize_url(&request.jira_base_url, "Jira site")?;
    let hashmal_mcp_url = normalize_url(&request.hashmal_mcp_url, "Hashmal MCP")?;
    let jira_email = normalize_email(&request.jira_email)?;
    let jql = normalize_jql(&request.jql)?;
    let encrypted_jira_token = next_envelope(
        Service::Jira,
        "Jira API",
        &current.jira_base_url,
        current.encrypted_jira_token.as_ref(),
        &jira_base_url,
        request.jira_token.as_deref(),
        store,
    )?;
    if encrypted_jira_token.is_some() && jira_email.is_empty() {
        return Err(ChngError::MissingEmail);
    }
    let encrypted_hashmal_token = next_envelope(
        Service::Hashmal,
        "Hashmal",
        &current.hashmal_mcp_url,
        current.encrypted_hashmal_token.as_ref(),
        &hashmal_mcp_url,
        request.hashmal_token.as_deref(),
        store,
    )?;
    Ok(ChngSettingsRow {
        jira_base_url,
        jira_email,
        jql,
        encrypted_jira_token,
        hashmal_mcp_url,
        encrypted_hashmal_token,
        updated_at: None,
    })
}

fn decrypt(
    store: &McpGatewaySecretStore,
    service: Service,
    url: &str,
    envelope: &str,
) -> Option<String> {
    store
        .decrypt(envelope, &service.binding(url))
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
}

fn view(row: &ChngSettingsRow, store: Option<&McpGatewaySecretStore>) -> ChngSettings {
    let reference = |service, url: &str, envelope: Option<&String>| {
        let plaintext = decrypt(store?, service, url, envelope?)?;
        normalize_secret_reference(&plaintext)
    };
    ChngSettings {
        jira_base_url: row.jira_base_url.clone(),
        jira_email: row.jira_email.clone(),
        jql: effective_jql(&row.jql),
        default_jql: DEFAULT_JQL.to_owned(),
        has_jira_token: row.encrypted_jira_token.is_some(),
        jira_token_reference: reference(
            Service::Jira,
            &row.jira_base_url,
            row.encrypted_jira_token.as_ref(),
        ),
        hashmal_mcp_url: row.hashmal_mcp_url.clone(),
        has_hashmal_token: row.encrypted_hashmal_token.is_some(),
        hashmal_token_reference: reference(
            Service::Hashmal,
            &row.hashmal_mcp_url,
            row.encrypted_hashmal_token.as_ref(),
        ),
        updated_at: row.updated_at,
    }
}

fn has_secrets(row: &ChngSettingsRow) -> bool {
    row.encrypted_jira_token.is_some() || row.encrypted_hashmal_token.is_some()
}

pub async fn get_settings(pool: &SqlitePool) -> Result<ChngSettings, ChngError> {
    let row = ChngSettingsRow::get(pool).await?.unwrap_or_default();
    // A row with no token never touches the key file. An unreadable key still
    // lists the row (tokens shown as stored) so it can be replaced.
    let store = if has_secrets(&row) {
        load_store().ok()
    } else {
        None
    };
    Ok(view(&row, store.as_ref()))
}

pub async fn update_settings(
    pool: &SqlitePool,
    request: UpdateChngSettingsRequest,
) -> Result<ChngSettings, ChngError> {
    let current = ChngSettingsRow::get(pool).await?;
    let mut store = None;
    let next = apply_update(current.as_ref(), &request, &mut store)?;
    if !ChngSettingsRow::save_if_unchanged(pool, current.as_ref(), &next).await? {
        return Err(ChngError::Conflict);
    }
    get_settings(pool).await
}

/// The plaintext token for `service`, resolving a 1Password reference with
/// the server's own credentials. `None` when no token is saved.
async fn token(
    row: &ChngSettingsRow,
    service: Service,
) -> Result<Option<(String, CredentialSource)>, ChngError> {
    let (url, envelope, label) = match service {
        Service::Jira => (&row.jira_base_url, &row.encrypted_jira_token, "Jira API"),
        Service::Hashmal => (
            &row.hashmal_mcp_url,
            &row.encrypted_hashmal_token,
            "Hashmal",
        ),
    };
    let Some(envelope) = envelope else {
        return Ok(None);
    };
    let store = load_store()?;
    let plaintext =
        decrypt(&store, service, url, envelope).ok_or(ChngError::Undecryptable(label))?;
    match normalize_secret_reference(&plaintext) {
        Some(reference) => resolve_reference(reference)
            .await
            .map(|value| Some((value, CredentialSource::OnePassword)))
            .map_err(|source| ChngError::Resolve {
                service: label,
                source,
            }),
        None => Ok(Some((plaintext, CredentialSource::Saved))),
    }
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(UPSTREAM_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        // A token goes only to the URL it was saved with, never to a redirect.
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("vibe-kanban/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap_or_default()
}

fn host_of(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or_else(|| "the configured URL".to_owned())
}

fn bounded(text: &str, limit: usize) -> String {
    let text: String = text.chars().filter(|c| !c.is_control()).collect();
    let text = text.trim();
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let mut clipped: String = text.chars().take(limit).collect();
    clipped.push('…');
    clipped
}

async fn read_body(response: reqwest::Response) -> Option<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length as usize > MAX_RESPONSE_BYTES)
    {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    (bytes.len() <= MAX_RESPONSE_BYTES).then(|| bytes.to_vec())
}

// -------------------------------------------------------------------- Jira

fn parse_jira_time(value: Option<&Value>) -> Option<DateTime<Utc>> {
    let text = value?.as_str()?;
    DateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f%z")
        .or_else(|_| DateTime::parse_from_rfc3339(text))
        .ok()
        .map(|time| time.with_timezone(&Utc))
}

/// One `/search/jql` page → tickets and the next page token.
fn parse_search_page(base_url: &str, page: &Value) -> (Vec<ChngTicket>, Option<String>) {
    let tickets = page
        .get("issues")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|issue| {
            let key = issue.get("key")?.as_str()?.to_owned();
            let fields = issue.get("fields");
            let field = |name: &str| fields.and_then(|fields| fields.get(name));
            let status = field("status");
            Some(ChngTicket {
                url: format!("{base_url}/browse/{key}"),
                key,
                summary: field("summary")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                status: status
                    .and_then(|status| status.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                status_category: status
                    .and_then(|status| status.get("statusCategory"))
                    .and_then(|category| category.get("key"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                priority: field("priority")
                    .and_then(|priority| priority.get("name"))
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                created: parse_jira_time(field("created")),
                updated: parse_jira_time(field("updated")),
            })
        })
        .collect();
    let is_last = page.get("isLast").and_then(Value::as_bool).unwrap_or(false);
    let next = page
        .get("nextPageToken")
        .and_then(Value::as_str)
        .filter(|token| !token.is_empty() && !is_last)
        .map(str::to_owned);
    (tickets, next)
}

/// Jira's `errorMessages` for a rejected search are JQL feedback the user
/// needs (and contain no credential), bounded.
fn jira_search_feedback(body: &[u8]) -> String {
    let messages = serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|body| {
            let messages: Vec<String> = body
                .get("errorMessages")?
                .as_array()?
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
            (!messages.is_empty()).then(|| messages.join("; "))
        })
        .unwrap_or_else(|| "check the JQL in Settings → CHNG".to_owned());
    bounded(&messages, MAX_UPSTREAM_MESSAGE)
}

fn jira_configured(row: &ChngSettingsRow) -> bool {
    !row.jira_base_url.is_empty()
        && !row.jira_email.is_empty()
        && row.encrypted_jira_token.is_some()
}

pub async fn search_tickets(pool: &SqlitePool) -> Result<ChngTicketList, ChngError> {
    let row = ChngSettingsRow::get(pool).await?.unwrap_or_default();
    let jql = effective_jql(&row.jql);
    if !jira_configured(&row) {
        return Ok(ChngTicketList {
            configured: false,
            jql,
            tickets: Vec::new(),
            truncated: false,
        });
    }
    let (token, source) = token(&row, Service::Jira)
        .await?
        .ok_or(ChngError::Undecryptable("Jira API"))?;
    let (tickets, truncated) = search_jira(
        &http_client(),
        &row.jira_base_url,
        &row.jira_email,
        &token,
        source,
        &jql,
    )
    .await?;
    Ok(ChngTicketList {
        configured: true,
        jql,
        tickets,
        truncated,
    })
}

async fn search_jira(
    http: &reqwest::Client,
    base_url: &str,
    email: &str,
    token: &str,
    source: CredentialSource,
    jql: &str,
) -> Result<(Vec<ChngTicket>, bool), ChngError> {
    let host = host_of(base_url);
    let endpoint = format!("{base_url}/rest/api/3/search/jql");
    let mut tickets = Vec::new();
    let mut next_page: Option<String> = None;
    for page_index in 0..JIRA_MAX_PAGES {
        let mut query = vec![
            ("jql", jql.to_owned()),
            ("maxResults", JIRA_PAGE_SIZE.to_string()),
            (
                "fields",
                "summary,status,priority,created,updated".to_owned(),
            ),
        ];
        if let Some(token) = &next_page {
            query.push(("nextPageToken", token.clone()));
        }
        let response = http
            .get(&endpoint)
            .query(&query)
            .basic_auth(email, Some(token))
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|_| ChngError::JiraUnreachable(host.clone()))?;
        let status = response.status();
        let body = read_body(response).await;
        match status.as_u16() {
            200 => {}
            401 | 403 => {
                return Err(ChngError::JiraRejected {
                    host,
                    credential: source,
                    status: status.as_u16(),
                });
            }
            400 => {
                return Err(ChngError::JiraSearch(jira_search_feedback(
                    body.as_deref().unwrap_or_default(),
                )));
            }
            404 => return Err(ChngError::JiraNotCloud(host)),
            429 => return Err(ChngError::JiraRateLimited),
            other => {
                return Err(ChngError::JiraUnexpected {
                    host,
                    status: other,
                });
            }
        }
        let page: Value = body
            .and_then(|body| serde_json::from_slice(&body).ok())
            .ok_or_else(|| ChngError::JiraUnexpected {
                host: host.clone(),
                status: status.as_u16(),
            })?;
        let (page_tickets, next) = parse_search_page(base_url, &page);
        tickets.extend(page_tickets);
        match next {
            Some(token) if page_index + 1 < JIRA_MAX_PAGES => next_page = Some(token),
            Some(_) => return Ok((tickets, true)),
            None => return Ok((tickets, false)),
        }
    }
    Ok((tickets, false))
}

// ----------------------------------------------------------------- Hashmal

/// Jira-style issue keys only, e.g. `CSEQ-1234`.
pub fn is_valid_ticket_key(key: &str) -> bool {
    let Some((project, number)) = key.split_once('-') else {
        return false;
    };
    key.len() <= 64
        && project.starts_with(|c: char| c.is_ascii_uppercase())
        && project
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        && !number.is_empty()
        && number.chars().all(|c| c.is_ascii_digit())
}

/// Hashmal session ids are Claude transcript file stems.
pub fn is_valid_session_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        && !id.starts_with('.')
}

fn clip_turn_text(text: &str) -> (String, bool) {
    if text.chars().count() <= MAX_TURN_CHARS {
        (text.to_owned(), false)
    } else {
        (text.chars().take(MAX_TURN_CHARS).collect(), true)
    }
}

fn parse_session(value: &Value) -> Option<HashmalSession> {
    let session_id = value.get("session_id")?.as_str()?;
    if !is_valid_session_id(session_id) {
        return None;
    }
    let text = |name: &str| value.get(name).and_then(Value::as_str).map(str::to_owned);
    Some(HashmalSession {
        session_id: session_id.to_owned(),
        title: bounded(&text("title").unwrap_or_else(|| session_id.to_owned()), 200),
        cwd: text("cwd").map(|cwd| bounded(&cwd, 500)),
        last_active: text("last_active")
            .and_then(|time| DateTime::parse_from_rfc3339(&time).ok())
            .map(|time| time.with_timezone(&Utc)),
        running: value
            .get("running")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn parse_transcript(value: &Value) -> Option<HashmalTranscript> {
    let session_id = value.get("session_id")?.as_str()?;
    let turns: Vec<HashmalTurn> = value
        .get("turns")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|turn| {
            let role = turn.get("role")?.as_str()?.to_owned();
            let (text, clipped) =
                clip_turn_text(turn.get("text").and_then(Value::as_str).unwrap_or(""));
            Some(HashmalTurn {
                role,
                text,
                tool: turn
                    .get("tool")
                    .and_then(Value::as_str)
                    .map(|tool| bounded(tool, 100)),
                truncated: clipped
                    || turn
                        .get("truncated")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
            })
        })
        .collect();
    // Keep the newest turns when Hashmal sends more than we show.
    let turns = turns[turns.len().saturating_sub(MAX_TURNS)..].to_vec();
    let total_turns = value
        .get("total_turns")
        .and_then(Value::as_u64)
        .map(|total| total.min(u32::MAX as u64) as u32)
        .unwrap_or(turns.len() as u32);
    Some(HashmalTranscript {
        session_id: session_id.to_owned(),
        total_turns,
        turns,
    })
}

/// Hashmal's tool payload → lookup. A payload that doesn't parse is
/// `unavailable`, never a guess.
fn parse_tool_payload(payload: &Value) -> HashmalChngLookup {
    let Some(tracked) = payload.get("tracked").and_then(Value::as_bool) else {
        return HashmalChngLookup::unavailable("Hashmal returned an unexpected CHNG lookup");
    };
    if !tracked {
        return HashmalChngLookup::empty(
            HashmalLookupState::NotTracked,
            Some("Hashmal hasn't synced this CHNG ticket yet".to_owned()),
        );
    }
    let sessions: Vec<HashmalSession> = payload
        .get("sessions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(parse_session)
        .take(MAX_SESSIONS)
        .collect();
    if sessions.is_empty() {
        return HashmalChngLookup::empty(
            HashmalLookupState::NoSessions,
            Some("Hashmal has no session linked to this ticket".to_owned()),
        );
    }
    let transcript = payload
        .get("transcript")
        .filter(|value| !value.is_null())
        .and_then(parse_transcript)
        .filter(|transcript| {
            sessions
                .iter()
                .any(|session| session.session_id == transcript.session_id)
        });
    HashmalChngLookup {
        state: HashmalLookupState::Found,
        message: None,
        manual: payload
            .get("manual")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        sessions,
        transcript,
    }
}

/// A JSON-RPC reply from Hashmal's `/mcp` → lookup.
fn parse_rpc_reply(reply: &Value) -> HashmalChngLookup {
    if let Some(error) = reply.get("error") {
        let message = error.get("message").and_then(Value::as_str).unwrap_or("");
        if message.starts_with("unknown tool") {
            return HashmalChngLookup::unavailable(format!(
                "This Hashmal doesn't have the CHNG lookup ({HASHMAL_TOOL}) yet; update Hashmal"
            ));
        }
        return HashmalChngLookup::unavailable(format!(
            "Hashmal refused the CHNG lookup: {}",
            bounded(message, MAX_UPSTREAM_MESSAGE)
        ));
    }
    let Some(result) = reply.get("result") else {
        return HashmalChngLookup::unavailable("Hashmal returned an unexpected reply");
    };
    // Prefer structured content; older servers only send the JSON as text.
    let payload = result
        .get("structuredContent")
        .cloned()
        .or_else(|| {
            result
                .get("content")?
                .as_array()?
                .iter()
                .find_map(|block| block.get("text")?.as_str())
                .and_then(|text| serde_json::from_str::<Value>(text).ok())
        })
        .unwrap_or(Value::Null);
    if result.get("isError").and_then(Value::as_bool) == Some(true) {
        let message = payload
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        return HashmalChngLookup::unavailable(format!(
            "Hashmal couldn't look up this ticket: {}",
            bounded(message, MAX_UPSTREAM_MESSAGE)
        ));
    }
    parse_tool_payload(&payload)
}

fn hashmal_request(key: &str, session_id: Option<&str>) -> Value {
    let mut arguments = json!({ "key": key, "limit": HASHMAL_TRANSCRIPT_TURNS });
    if let Some(session_id) = session_id {
        arguments["session_id"] = json!(session_id);
    }
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": HASHMAL_TOOL, "arguments": arguments },
    })
}

async fn call_hashmal(
    http: &reqwest::Client,
    url: &str,
    token: &str,
    source: CredentialSource,
    key: &str,
    session_id: Option<&str>,
) -> HashmalChngLookup {
    let host = host_of(url);
    let response = match http
        .post(url)
        .bearer_auth(token)
        .header(
            reqwest::header::ACCEPT,
            "application/json, text/event-stream",
        )
        .header("MCP-Protocol-Version", "2025-06-18")
        .json(&hashmal_request(key, session_id))
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => {
            return HashmalChngLookup::unavailable(format!(
                "Hashmal at {host} is unreachable or timed out"
            ));
        }
    };
    let status = response.status().as_u16();
    match status {
        401 | 403 => {
            return HashmalChngLookup::unavailable(format!(
                "Hashmal at {host} rejected {source} (HTTP {status})"
            ));
        }
        404 => {
            return HashmalChngLookup::unavailable(format!(
                "Hashmal at {host} has no MCP endpoint at this URL"
            ));
        }
        200 | 400 => {}
        _ => {
            return HashmalChngLookup::unavailable(format!(
                "Hashmal at {host} answered HTTP {status}"
            ));
        }
    }
    match read_body(response)
        .await
        .and_then(|body| serde_json::from_slice::<Value>(&body).ok())
    {
        Some(reply) => parse_rpc_reply(&reply),
        None => HashmalChngLookup::unavailable(format!(
            "Hashmal at {host} returned a response that isn't MCP JSON (is it behind a login page?)"
        )),
    }
}

/// Asks Hashmal which sessions worked `key`, with the transcript of
/// `session_id` (one of them) or of the best match.
pub async fn hashmal_lookup(
    pool: &SqlitePool,
    key: &str,
    session_id: Option<&str>,
) -> HashmalChngLookup {
    let row = match ChngSettingsRow::get(pool).await {
        Ok(row) => row.unwrap_or_default(),
        Err(_) => return HashmalChngLookup::unavailable("CHNG settings could not be read"),
    };
    if row.hashmal_mcp_url.is_empty() || row.encrypted_hashmal_token.is_none() {
        return HashmalChngLookup::empty(
            HashmalLookupState::NotConfigured,
            Some("Hashmal isn't set up in Settings → CHNG".to_owned()),
        );
    }
    let (token, source) = match token(&row, Service::Hashmal).await {
        Ok(Some(token)) => token,
        Ok(None) => {
            return HashmalChngLookup::empty(HashmalLookupState::NotConfigured, None);
        }
        Err(error) => return HashmalChngLookup::unavailable(error.to_string()),
    };
    call_hashmal(
        &http_client(),
        &row.hashmal_mcp_url,
        &token,
        source,
        key,
        session_id,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
        pool
    }

    fn temp_store() -> (tempfile::TempDir, McpGatewaySecretStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = McpGatewaySecretStore::load_or_generate(&dir.path().join("key")).unwrap();
        (dir, store)
    }

    fn request(jira_url: &str, jira_token: Option<&str>) -> UpdateChngSettingsRequest {
        UpdateChngSettingsRequest {
            jira_base_url: jira_url.into(),
            jira_email: "ops@example.com".into(),
            jql: String::new(),
            jira_token: jira_token.map(str::to_owned),
            hashmal_mcp_url: String::new(),
            hashmal_token: None,
        }
    }

    #[test]
    fn urls_require_https_and_reject_credentials() {
        assert_eq!(
            normalize_url(" https://acme.atlassian.net/ ", "Jira site").unwrap(),
            "https://acme.atlassian.net"
        );
        assert_eq!(
            normalize_url("https://tickets.example.dev/mcp", "Hashmal MCP").unwrap(),
            "https://tickets.example.dev/mcp"
        );
        assert_eq!(
            normalize_url("http://127.0.0.1:9000", "Jira site").unwrap(),
            "http://127.0.0.1:9000"
        );
        assert_eq!(normalize_url("  ", "Jira site").unwrap(), "");
        for bad in [
            "http://acme.atlassian.net",
            "https://user:pw@acme.atlassian.net",
            "https://acme.atlassian.net/?x=1",
            "https://acme.atlassian.net/#frag",
            "ftp://acme",
            "not a url",
        ] {
            assert!(normalize_url(bad, "Jira site").is_err(), "{bad}");
        }
    }

    #[test]
    fn jql_default_is_stored_empty_and_bounded() {
        assert_eq!(normalize_jql(DEFAULT_JQL).unwrap(), "");
        assert_eq!(normalize_jql("  project = X ").unwrap(), "project = X");
        assert!(normalize_jql(&"x".repeat(MAX_JQL_CHARS + 1)).is_err());
        assert!(normalize_jql("a\0b").is_err());
        assert_eq!(effective_jql(""), DEFAULT_JQL);
        assert_eq!(effective_jql("project = X"), "project = X");
    }

    #[test]
    fn ticket_keys_and_session_ids_are_validated() {
        for good in ["CSEQ-1", "AB_2-123", "X-9"] {
            assert!(is_valid_ticket_key(good), "{good}");
        }
        for bad in ["cseq-1", "CSEQ-", "-1", "CSEQ-1a", "CSEQ", "1A-2", "A/B-1"] {
            assert!(!is_valid_ticket_key(bad), "{bad}");
        }
        assert!(is_valid_session_id("6f1c2b7e-1234-4abc-9def-0123456789ab"));
        for bad in ["", "../x", "a/b", ".hidden", &"a".repeat(129)] {
            assert!(!is_valid_session_id(bad), "{bad}");
        }
    }

    #[tokio::test]
    async fn tokens_are_write_only_and_bound_to_their_url() {
        let pool = test_pool().await;
        let (_dir, store) = temp_store();
        let mut cached = Some(store.clone());

        let first = apply_update(
            None,
            &request("https://acme.atlassian.net", Some("synthetic-literal")),
            &mut cached,
        )
        .unwrap();
        assert!(
            ChngSettingsRow::save_if_unchanged(&pool, None, &first)
                .await
                .unwrap()
        );
        let saved = ChngSettingsRow::get(&pool).await.unwrap().unwrap();
        let shown = view(&saved, Some(&store));
        assert!(shown.has_jira_token);
        assert_eq!(shown.jira_token_reference, None);
        assert!(
            !serde_json::to_string(&shown)
                .unwrap()
                .contains("synthetic-literal")
        );
        assert_eq!(shown.jql, DEFAULT_JQL);

        // Same URL, no token submitted: kept.
        let kept = apply_update(
            Some(&saved),
            &request("https://acme.atlassian.net/", None),
            &mut cached,
        )
        .unwrap();
        assert_eq!(kept.encrypted_jira_token, saved.encrypted_jira_token);

        // New URL without re-entering the token: refused.
        let moved = apply_update(
            Some(&saved),
            &request("https://evil.example.com", None),
            &mut cached,
        );
        assert!(matches!(moved, Err(ChngError::ReenterToken("Jira API"))));

        // The envelope only decrypts for its own URL.
        let envelope = saved.encrypted_jira_token.as_deref().unwrap();
        assert!(decrypt(&store, Service::Jira, "https://evil.example.com", envelope).is_none());
        assert_eq!(
            decrypt(
                &store,
                Service::Jira,
                "https://acme.atlassian.net",
                envelope
            )
            .as_deref(),
            Some("synthetic-literal")
        );

        // An empty string clears; clearing the URL drops the token.
        let cleared = apply_update(
            Some(&saved),
            &request("https://acme.atlassian.net", Some("  ")),
            &mut cached,
        )
        .unwrap();
        assert_eq!(cleared.encrypted_jira_token, None);
        let unset = apply_update(Some(&saved), &request("", None), &mut cached).unwrap();
        assert_eq!(unset.encrypted_jira_token, None);
    }

    #[tokio::test]
    async fn references_are_shown_and_email_is_required() {
        let (_dir, store) = temp_store();
        let mut cached = Some(store.clone());
        let row = apply_update(
            None,
            &request(
                "https://acme.atlassian.net",
                Some("\"op://Homelab/Jira API/api_token\""),
            ),
            &mut cached,
        )
        .unwrap();
        assert_eq!(
            view(&row, Some(&store)).jira_token_reference.as_deref(),
            Some("op://Homelab/Jira API/api_token")
        );
        let mut no_email = request("https://acme.atlassian.net", Some("x"));
        no_email.jira_email = String::new();
        assert!(matches!(
            apply_update(None, &no_email, &mut cached),
            Err(ChngError::MissingEmail)
        ));
    }

    #[tokio::test]
    async fn concurrent_destination_change_is_a_conflict() {
        let pool = test_pool().await;
        let (_dir, store) = temp_store();
        let mut cached = Some(store);
        let first = apply_update(
            None,
            &request("https://acme.atlassian.net", Some("t1")),
            &mut cached,
        )
        .unwrap();
        assert!(
            ChngSettingsRow::save_if_unchanged(&pool, None, &first)
                .await
                .unwrap()
        );
        // A second "first save" lost the race.
        assert!(
            !ChngSettingsRow::save_if_unchanged(&pool, None, &first)
                .await
                .unwrap()
        );
        let saved = ChngSettingsRow::get(&pool).await.unwrap().unwrap();
        let mut stale = saved.clone();
        stale.jira_base_url = "https://other.atlassian.net".into();
        assert!(
            !ChngSettingsRow::save_if_unchanged(&pool, Some(&stale), &saved)
                .await
                .unwrap()
        );
        assert!(
            ChngSettingsRow::save_if_unchanged(&pool, Some(&saved), &saved)
                .await
                .unwrap()
        );
    }

    #[test]
    fn jira_pages_parse_and_report_next_token() {
        let page = json!({
            "issues": [{
                "key": "CSEQ-7",
                "fields": {
                    "summary": "Rotate the VPN cert",
                    "status": {"name": "In Review", "statusCategory": {"key": "indeterminate"}},
                    "priority": {"name": "High"},
                    "created": "2026-11-01T12:00:00.000+0000",
                    "updated": "2026-11-02T08:30:00.000-0500"
                }
            }, {"fields": {}}],
            "nextPageToken": "abc",
            "isLast": false
        });
        let (tickets, next) = parse_search_page("https://acme.atlassian.net", &page);
        assert_eq!(tickets.len(), 1);
        let ticket = &tickets[0];
        assert_eq!(ticket.url, "https://acme.atlassian.net/browse/CSEQ-7");
        assert_eq!(ticket.status_category, "indeterminate");
        assert_eq!(ticket.priority.as_deref(), Some("High"));
        assert_eq!(
            ticket.updated.unwrap().to_rfc3339(),
            "2026-11-02T13:30:00+00:00"
        );
        assert_eq!(next.as_deref(), Some("abc"));
        let (_, last) = parse_search_page(
            "https://x",
            &json!({"issues": [], "isLast": true, "nextPageToken": "abc"}),
        );
        assert_eq!(last, None);
    }

    #[test]
    fn jira_feedback_is_bounded_and_falls_back() {
        let body = json!({"errorMessages": ["Field 'issuetype' does not exist"]}).to_string();
        assert_eq!(
            jira_search_feedback(body.as_bytes()),
            "Field 'issuetype' does not exist"
        );
        assert_eq!(
            jira_search_feedback(b"<html>"),
            "check the JQL in Settings \u{2192} CHNG"
        );
        let long = json!({"errorMessages": ["x".repeat(1000)]}).to_string();
        assert!(jira_search_feedback(long.as_bytes()).chars().count() <= MAX_UPSTREAM_MESSAGE + 1);
    }

    fn found_payload() -> Value {
        json!({
            "key": "CSEQ-7",
            "tracked": true,
            "manual": true,
            "sessions": [
                {"session_id": "s-1", "title": "Draft CHNG", "cwd": "/w", "last_active": "2026-11-01T12:00:00+00:00", "running": true},
                {"session_id": "../bad", "title": "skip"},
                {"session_id": "s-2", "title": "Follow-up"}
            ],
            "transcript": {"session_id": "s-1", "total_turns": 90, "turns": [
                {"role": "user", "text": "file the CHNG", "truncated": false},
                {"role": "tool", "text": "x".repeat(MAX_TURN_CHARS + 10), "tool": "createJiraIssue"}
            ]}
        })
    }

    #[test]
    fn hashmal_payloads_map_to_states() {
        let found = parse_tool_payload(&found_payload());
        assert_eq!(found.state, HashmalLookupState::Found);
        assert!(found.manual);
        assert_eq!(
            found
                .sessions
                .iter()
                .map(|s| s.session_id.as_str())
                .collect::<Vec<_>>(),
            ["s-1", "s-2"]
        );
        assert!(found.sessions[0].running);
        let transcript = found.transcript.unwrap();
        assert_eq!(transcript.total_turns, 90);
        assert!(transcript.turns[1].truncated);
        assert_eq!(transcript.turns[1].text.chars().count(), MAX_TURN_CHARS);

        let untracked = parse_tool_payload(&json!({"tracked": false, "sessions": []}));
        assert_eq!(untracked.state, HashmalLookupState::NotTracked);
        let none = parse_tool_payload(&json!({"tracked": true, "sessions": []}));
        assert_eq!(none.state, HashmalLookupState::NoSessions);
        let junk = parse_tool_payload(&json!({"oops": 1}));
        assert_eq!(junk.state, HashmalLookupState::Unavailable);

        // A transcript for a session that isn't listed is dropped.
        let mut stray = found_payload();
        stray["transcript"]["session_id"] = json!("elsewhere");
        assert!(parse_tool_payload(&stray).transcript.is_none());
    }

    #[test]
    fn hashmal_rpc_errors_are_classified() {
        let unknown = parse_rpc_reply(&json!({"jsonrpc": "2.0", "id": 1,
            "error": {"code": -32602, "message": "unknown tool: hashmal_chng_sessions"}}));
        assert_eq!(unknown.state, HashmalLookupState::Unavailable);
        assert!(unknown.message.unwrap().contains("update Hashmal"));

        let tool_error = parse_rpc_reply(&json!({"jsonrpc": "2.0", "id": 1, "result": {
            "isError": true, "structuredContent": {"error": "session is not linked"}}}));
        assert!(
            tool_error
                .message
                .unwrap()
                .contains("session is not linked")
        );

        let text_only = parse_rpc_reply(&json!({"jsonrpc": "2.0", "id": 1, "result": {
            "content": [{"type": "text", "text": found_payload().to_string()}]}}));
        assert_eq!(text_only.state, HashmalLookupState::Found);
    }

    #[test]
    fn hashmal_request_names_the_tool_and_optional_session() {
        let plain = hashmal_request("CSEQ-7", None);
        assert_eq!(plain["params"]["name"], HASHMAL_TOOL);
        assert!(plain["params"]["arguments"].get("session_id").is_none());
        let targeted = hashmal_request("CSEQ-7", Some("s-2"));
        assert_eq!(targeted["params"]["arguments"]["session_id"], "s-2");
    }

    async fn fake_server(
        status: &'static str,
        body: String,
    ) -> (String, tokio::task::JoinHandle<String>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = vec![0; 16384];
            let n = socket.read(&mut bytes).await.unwrap();
            let request = String::from_utf8_lossy(&bytes[..n]).to_string();
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            request
        });
        (url, task)
    }

    #[tokio::test]
    async fn jira_search_sends_basic_auth_and_classifies_rejection() {
        let page =
            json!({"issues": [{"key": "CSEQ-1", "fields": {"summary": "s"}}], "isLast": true});
        let (url, task) = fake_server("200 OK", page.to_string()).await;
        let (tickets, truncated) = search_jira(
            &http_client(),
            &url,
            "ops@example.com",
            "synthetic-token",
            CredentialSource::Saved,
            DEFAULT_JQL,
        )
        .await
        .unwrap();
        assert_eq!(tickets.len(), 1);
        assert!(!truncated);
        let request = task.await.unwrap();
        assert!(request.starts_with("GET /rest/api/3/search/jql?jql=project"));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: basic ")
        );

        let (url, _task) = fake_server("401 Unauthorized", "{}".into()).await;
        let error = search_jira(
            &http_client(),
            &url,
            "ops@example.com",
            "synthetic-token",
            CredentialSource::OnePassword,
            DEFAULT_JQL,
        )
        .await
        .unwrap_err();
        let message = error.to_string();
        assert!(message.contains("1Password reference"), "{message}");
        assert!(!message.contains("synthetic-token"));
    }

    #[tokio::test]
    async fn hashmal_call_sends_bearer_and_handles_login_pages() {
        let reply =
            json!({"jsonrpc": "2.0", "id": 1, "result": {"structuredContent": found_payload()}});
        let (url, task) = fake_server("200 OK", reply.to_string()).await;
        let lookup = call_hashmal(
            &http_client(),
            &format!("{url}/mcp"),
            "hsm_synthetic",
            CredentialSource::Saved,
            "CSEQ-7",
            None,
        )
        .await;
        assert_eq!(lookup.state, HashmalLookupState::Found);
        let request = task.await.unwrap();
        assert!(request.starts_with("POST /mcp "));
        assert!(request.contains("hashmal_chng_sessions"));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer hsm_synthetic")
        );

        let (url, _task) = fake_server("200 OK", "<html>login</html>".into()).await;
        let lookup = call_hashmal(
            &http_client(),
            &url,
            "hsm_synthetic",
            CredentialSource::Saved,
            "CSEQ-7",
            None,
        )
        .await;
        assert_eq!(lookup.state, HashmalLookupState::Unavailable);
        assert!(lookup.message.unwrap().contains("login page"));
    }
}
