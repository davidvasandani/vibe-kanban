//! Embedded OAuth 2.1 authorization server for remote MCP clients that can
//! only authenticate with OAuth (ChatGPT custom connectors).
//!
//! VK issues and verifies the tokens itself; the MCP transport is still the
//! deployment's supergateway, which the reverse proxy admits only after
//! `forward_auth` to [`verify`] succeeds. Everything is inert (404) unless
//! `VK_MCP_OAUTH_PUBLIC_URL` names the public issuer. See
//! `crates/mcp/AGENTS.md` ("Remote OAuth access").

pub mod config;
pub mod crypto;
pub mod pages;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Form, Path, Query, State, rejection::FormRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{any, get, post},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{DateTime, Duration, Utc};
use db::models::mcp_oauth::{
    ExchangeOutcome, IssuedTokenHashes, McpOAuth, McpOAuthGrantSummary, NewMcpOAuthAuthorization,
    NewMcpOAuthClient, RotateOutcome, timestamp,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::SqlitePool;
use url::Url;
use utils::response::ApiResponse;
use uuid::Uuid;

pub use self::config::McpOAuthConfig;
use self::crypto::*;

pub const SCOPE: &str = "mcp";
const CONSENT_TTL: Duration = Duration::minutes(10);
const CODE_TTL: Duration = Duration::seconds(60);
const ACCESS_TTL: Duration = Duration::hours(1);
const REFRESH_TTL: Duration = Duration::days(30);
const TOUCH_INTERVAL: Duration = Duration::minutes(1);

const MAX_CLIENTS: i64 = 200;
const MAX_REGISTRATION_BYTES: usize = 16 * 1024;
const MAX_FORM_BYTES: usize = 8 * 1024;
const MAX_REDIRECT_URIS: usize = 10;
const MAX_URI_LEN: usize = 2048;
const MAX_CLIENT_NAME_CHARS: usize = 200;
const MAX_STATE_LEN: usize = 1024;

#[derive(Clone)]
pub struct McpOAuthState {
    pub pool: SqlitePool,
    pub config: Option<Arc<McpOAuthConfig>>,
}

impl McpOAuthState {
    pub fn new(pool: SqlitePool, config: Option<McpOAuthConfig>) -> Self {
        Self {
            pool,
            config: config.map(Arc::new),
        }
    }

    fn enabled(&self) -> Option<&McpOAuthConfig> {
        self.config.as_deref()
    }
}

/// Discovery, registration, consent and token routes, mounted at the host
/// root (before the SPA fallback).
pub fn public_router() -> Router<McpOAuthState> {
    Router::new()
        .route(
            "/.well-known/oauth-protected-resource",
            get(protected_resource_metadata),
        )
        .route(
            "/.well-known/oauth-protected-resource/oauth/mcp",
            get(protected_resource_metadata),
        )
        .route(
            "/.well-known/oauth-authorization-server",
            get(authorization_server_metadata),
        )
        .route(
            "/.well-known/oauth-authorization-server/oauth/mcp",
            get(authorization_server_metadata),
        )
        .route(
            "/oauth/register",
            post(register).layer(DefaultBodyLimit::max(MAX_REGISTRATION_BYTES)),
        )
        .route(
            "/oauth/authorize",
            get(authorize_page)
                .post(authorize_decision)
                .layer(DefaultBodyLimit::max(MAX_FORM_BYTES)),
        )
        .route(
            "/oauth/token",
            post(token).layer(DefaultBodyLimit::max(MAX_FORM_BYTES)),
        )
}

/// Routes nested under `/api/mcp-oauth`: the reverse proxy's token check and
/// owner-facing grant management (behind the host's own login).
pub fn api_router() -> Router<McpOAuthState> {
    Router::new()
        .route("/verify", any(verify))
        .route("/grants", get(list_grants))
        .route("/grants/{id}", axum::routing::delete(revoke_grant))
}

// ---------------------------------------------------------------------------
// Discovery

async fn protected_resource_metadata(State(state): State<McpOAuthState>) -> Response {
    let Some(config) = state.enabled() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    Json(json!({
        "resource": config.resource(),
        "authorization_servers": [config.issuer],
        "scopes_supported": [SCOPE],
        "bearer_methods_supported": ["header"],
        "resource_name": "Vibe Kanban",
    }))
    .into_response()
}

async fn authorization_server_metadata(State(state): State<McpOAuthState>) -> Response {
    let Some(config) = state.enabled() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    Json(json!({
        "issuer": config.issuer,
        "authorization_endpoint": config.endpoint("/oauth/authorize"),
        "token_endpoint": config.endpoint("/oauth/token"),
        "registration_endpoint": config.endpoint("/oauth/register"),
        "scopes_supported": [SCOPE],
        "response_types_supported": ["code"],
        "response_modes_supported": ["query"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "token_endpoint_auth_methods_supported": AUTH_METHODS,
        "code_challenge_methods_supported": ["S256"],
        "authorization_response_iss_parameter_supported": true,
    }))
    .into_response()
}

// ---------------------------------------------------------------------------
// Dynamic client registration (RFC 7591)

const AUTH_METHODS: [&str; 3] = ["none", "client_secret_post", "client_secret_basic"];

#[derive(Debug, Deserialize)]
struct RegistrationRequest {
    redirect_uris: Option<Vec<String>>,
    client_name: Option<String>,
    token_endpoint_auth_method: Option<String>,
    grant_types: Option<Vec<String>>,
    response_types: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct RegistrationResponse {
    client_id: String,
    client_id_issued_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_secret: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_secret_expires_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_name: Option<String>,
    redirect_uris: Vec<String>,
    grant_types: [&'static str; 2],
    response_types: [&'static str; 1],
    token_endpoint_auth_method: String,
    scope: &'static str,
}

fn registration_error(code: &str, description: &str) -> Response {
    no_store(
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": code, "error_description": description })),
        )
            .into_response(),
    )
}

/// A redirect URI must be absolute `https` (or `http` on loopback for native
/// clients) without a fragment.
fn validate_redirect_uri(raw: &str) -> Result<(), &'static str> {
    if raw.len() > MAX_URI_LEN {
        return Err("redirect_uri is too long");
    }
    let url = Url::parse(raw).map_err(|_| "redirect_uri must be an absolute URL")?;
    if url.fragment().is_some() {
        return Err("redirect_uri must not contain a fragment");
    }
    match url.scheme() {
        "https" if url.host().is_some() => Ok(()),
        "http" if config::is_loopback(&url) => Ok(()),
        _ => Err("redirect_uri must use https (or http on a loopback host)"),
    }
}

struct ValidRegistration {
    redirect_uris: Vec<String>,
    client_name: Option<String>,
    method: String,
}

/// RFC 7591 error code and description.
type RegistrationError = (&'static str, &'static str);

fn validate_registration(
    request: &RegistrationRequest,
) -> Result<ValidRegistration, RegistrationError> {
    let redirect_uris = request.redirect_uris.clone().unwrap_or_default();
    if redirect_uris.is_empty() || redirect_uris.len() > MAX_REDIRECT_URIS {
        return Err((
            "invalid_redirect_uri",
            "between 1 and 10 redirect_uris are required",
        ));
    }
    for uri in &redirect_uris {
        if let Err(reason) = validate_redirect_uri(uri) {
            return Err(("invalid_redirect_uri", reason));
        }
    }
    let grant_types = request
        .grant_types
        .clone()
        .unwrap_or_else(|| vec!["authorization_code".to_string()]);
    if !grant_types.iter().any(|g| g == "authorization_code")
        || grant_types
            .iter()
            .any(|g| g != "authorization_code" && g != "refresh_token")
    {
        return Err((
            "invalid_client_metadata",
            "grant_types must be authorization_code (optionally with refresh_token)",
        ));
    }
    if let Some(response_types) = &request.response_types
        && response_types.iter().any(|r| r != "code")
    {
        return Err((
            "invalid_client_metadata",
            "only the code response type is supported",
        ));
    }
    let method = request
        .token_endpoint_auth_method
        .clone()
        .unwrap_or_else(|| "client_secret_basic".to_string());
    if !AUTH_METHODS.contains(&method.as_str()) {
        return Err((
            "invalid_client_metadata",
            "token_endpoint_auth_method must be none, client_secret_post or client_secret_basic",
        ));
    }
    let client_name = match request.client_name.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(name) if name.chars().count() > MAX_CLIENT_NAME_CHARS => {
            return Err(("invalid_client_metadata", "client_name is too long"));
        }
        Some(name) => Some(name.chars().filter(|c| !c.is_control()).collect()),
    };
    Ok(ValidRegistration {
        redirect_uris,
        client_name,
        method,
    })
}

async fn register(State(state): State<McpOAuthState>, body: Bytes) -> Response {
    if state.enabled().is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let request: RegistrationRequest = match serde_json::from_slice(&body) {
        Ok(request) => request,
        Err(_) => {
            return registration_error(
                "invalid_client_metadata",
                "the registration body must be a JSON object",
            );
        }
    };
    let ValidRegistration {
        redirect_uris,
        client_name,
        method,
    } = match validate_registration(&request) {
        Ok(valid) => valid,
        Err((code, description)) => return registration_error(code, description),
    };

    let now = Utc::now();
    if let Err(error) = McpOAuth::prune(&state.pool, now).await {
        return server_error("prune OAuth state", error);
    }
    let client_id = random_secret(16);
    let client_secret = (method != "none").then(random_token);
    let secret_hash = client_secret.as_deref().map(digest);
    match McpOAuth::insert_client(
        &state.pool,
        NewMcpOAuthClient {
            client_id: &client_id,
            client_secret_hash: secret_hash.as_deref(),
            token_endpoint_auth_method: &method,
            client_name: client_name.as_deref(),
            redirect_uris: &redirect_uris,
        },
        now,
        MAX_CLIENTS,
    )
    .await
    {
        Ok(true) => {}
        Ok(false) => {
            return registration_error(
                "invalid_client_metadata",
                "the client registration limit has been reached; revoke unused connections and retry later",
            );
        }
        Err(error) => return server_error("register OAuth client", error),
    }
    tracing::info!(client_id = %client_id, auth_method = %method, "Registered MCP OAuth client");

    let response = RegistrationResponse {
        client_id,
        client_id_issued_at: now.timestamp(),
        client_secret_expires_at: client_secret.as_ref().map(|_| 0),
        client_secret,
        client_name,
        redirect_uris,
        grant_types: ["authorization_code", "refresh_token"],
        response_types: ["code"],
        token_endpoint_auth_method: method,
        scope: SCOPE,
    };
    no_store((StatusCode::CREATED, Json(response)).into_response())
}

// ---------------------------------------------------------------------------
// Authorization endpoint

#[derive(Debug, Default, Deserialize)]
struct AuthorizeQuery {
    response_type: Option<String>,
    client_id: Option<String>,
    redirect_uri: Option<String>,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
    state: Option<String>,
    scope: Option<String>,
    resource: Option<String>,
}

/// Build the client redirect, preserving any query the client registered.
fn redirect_with(redirect_uri: &str, params: &[(&str, &str)]) -> Response {
    let Ok(mut url) = Url::parse(redirect_uri) else {
        return pages::error_page(StatusCode::BAD_REQUEST, "The redirect URI is invalid.");
    };
    {
        let mut pairs = url.query_pairs_mut();
        for (key, value) in params {
            pairs.append_pair(key, value);
        }
    }
    let Ok(location) = HeaderValue::from_str(url.as_str()) else {
        return pages::error_page(StatusCode::BAD_REQUEST, "The redirect URI is invalid.");
    };
    let mut response = StatusCode::FOUND.into_response();
    response.headers_mut().insert(header::LOCATION, location);
    pages::harden(response)
}

fn redirect_error(
    config: &McpOAuthConfig,
    redirect_uri: &str,
    state: Option<&str>,
    error: &str,
    description: &str,
) -> Response {
    let mut params = vec![("error", error), ("error_description", description)];
    if let Some(state) = state {
        params.push(("state", state));
    }
    params.push(("iss", config.issuer.as_str()));
    redirect_with(redirect_uri, &params)
}

/// `scope` may be absent (defaults to `mcp`) or name only `mcp`.
fn scope_is_supported(scope: Option<&str>) -> bool {
    scope.is_none_or(|scope| scope.split_whitespace().all(|s| s == SCOPE))
}

async fn authorize_page(
    State(state): State<McpOAuthState>,
    Query(query): Query<AuthorizeQuery>,
) -> Response {
    let Some(config) = state.enabled() else {
        return StatusCode::NOT_FOUND.into_response();
    };

    // Until the client and redirect URI are proven, never redirect anywhere.
    let Some(client_id) = query.client_id.as_deref() else {
        return pages::error_page(StatusCode::BAD_REQUEST, "The request has no client_id.");
    };
    let client = match McpOAuth::find_client(&state.pool, client_id).await {
        Ok(Some(client)) => client,
        Ok(None) => {
            return pages::error_page(
                StatusCode::BAD_REQUEST,
                "This application is not registered (or its registration expired).",
            );
        }
        Err(error) => return server_error("load OAuth client", error),
    };
    let registered = client.redirect_uris();
    let redirect_uri = match query.redirect_uri.as_deref() {
        Some(uri) if registered.iter().any(|r| r == uri) => uri.to_string(),
        None if registered.len() == 1 => registered[0].clone(),
        _ => {
            return pages::error_page(
                StatusCode::BAD_REQUEST,
                "The redirect URI does not match the application's registration.",
            );
        }
    };

    let client_state = query.state.as_deref();
    let fail = |error: &str, description: &str| {
        redirect_error(config, &redirect_uri, client_state, error, description)
    };
    if client_state.is_some_and(|s| s.len() > MAX_STATE_LEN) {
        // Do not echo an oversized state back.
        return redirect_error(
            config,
            &redirect_uri,
            None,
            "invalid_request",
            "state is too long",
        );
    }
    if query.response_type.as_deref() != Some("code") {
        return fail("unsupported_response_type", "response_type must be code");
    }
    let Some(code_challenge) = query
        .code_challenge
        .as_deref()
        .filter(|c| is_valid_s256_challenge(c))
    else {
        return fail("invalid_request", "a valid PKCE code_challenge is required");
    };
    if query.code_challenge_method.as_deref() != Some("S256") {
        return fail("invalid_request", "code_challenge_method must be S256");
    }
    if !scope_is_supported(query.scope.as_deref()) {
        return fail("invalid_scope", "only the mcp scope is supported");
    }
    let resource = config.resource();
    if query.resource.as_deref().is_some_and(|r| r != resource) {
        return fail(
            "invalid_target",
            "resource must be this server's MCP endpoint",
        );
    }

    let now = Utc::now();
    if let Err(error) = McpOAuth::prune(&state.pool, now).await {
        return server_error("prune OAuth state", error);
    }
    let request_id = Uuid::new_v4().to_string();
    let consent = random_token();
    let consent_hash = digest(&consent);
    if let Err(error) = McpOAuth::insert_authorization(
        &state.pool,
        NewMcpOAuthAuthorization {
            id: &request_id,
            client_id: &client.client_id,
            redirect_uri: &redirect_uri,
            code_challenge,
            scope: SCOPE,
            resource: &resource,
            state: client_state,
            consent_hash: &consent_hash,
            expires_at: now + CONSENT_TTL,
        },
        now,
    )
    .await
    {
        return server_error("store authorization request", error);
    }

    pages::consent_page(pages::ConsentView {
        client_name: client.client_name.as_deref(),
        client_id: &client.client_id,
        redirect_uri: &redirect_uri,
        request_id: &request_id,
        consent: &consent,
    })
}

#[derive(Debug, Deserialize)]
struct ConsentForm {
    request_id: String,
    consent: String,
    decision: String,
}

async fn authorize_decision(
    State(state): State<McpOAuthState>,
    form: Result<Form<ConsentForm>, FormRejection>,
) -> Response {
    let Some(config) = state.enabled() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(Form(form)) = form else {
        return pages::error_page(StatusCode::BAD_REQUEST, "The approval form was incomplete.");
    };
    let rejected = || {
        pages::error_page(
            StatusCode::BAD_REQUEST,
            "This approval link is invalid, expired or was already used.",
        )
    };
    let authorization = match McpOAuth::find_authorization(&state.pool, &form.request_id).await {
        Ok(Some(authorization)) => authorization,
        Ok(None) => return rejected(),
        Err(error) => return server_error("load authorization request", error),
    };
    if !digest_matches(&form.consent, &authorization.consent_hash) {
        return rejected();
    }

    let now = Utc::now();
    let approved = match form.decision.as_str() {
        "approve" => true,
        "deny" => false,
        _ => return pages::error_page(StatusCode::BAD_REQUEST, "Unknown decision."),
    };
    let code = approved.then(random_token);
    let code_hash = code.as_deref().map(digest);
    let resolved = McpOAuth::resolve_pending(
        &state.pool,
        &authorization.id,
        code_hash.as_deref().map(|hash| (hash, now + CODE_TTL)),
        now,
    )
    .await;
    match resolved {
        Ok(true) => {}
        Ok(false) => return rejected(),
        Err(error) => return server_error("resolve authorization request", error),
    }

    let client_state = authorization.state.as_deref();
    match code {
        Some(code) => {
            tracing::info!(client_id = %authorization.client_id, "Approved MCP OAuth authorization");
            let mut params = vec![("code", code.as_str())];
            if let Some(client_state) = client_state {
                params.push(("state", client_state));
            }
            params.push(("iss", config.issuer.as_str()));
            redirect_with(&authorization.redirect_uri, &params)
        }
        None => redirect_error(
            config,
            &authorization.redirect_uri,
            client_state,
            "access_denied",
            "the owner denied the request",
        ),
    }
}

// ---------------------------------------------------------------------------
// Token endpoint

#[derive(Debug, Deserialize)]
struct TokenRequest {
    grant_type: Option<String>,
    code: Option<String>,
    redirect_uri: Option<String>,
    code_verifier: Option<String>,
    refresh_token: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
    scope: Option<String>,
    resource: Option<String>,
}

fn no_store(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    response
}

fn token_error(status: StatusCode, code: &str, description: &str) -> Response {
    no_store(
        (
            status,
            Json(json!({ "error": code, "error_description": description })),
        )
            .into_response(),
    )
}

fn invalid_grant(description: &str) -> Response {
    token_error(StatusCode::BAD_REQUEST, "invalid_grant", description)
}

fn server_error(context: &str, error: sqlx::Error) -> Response {
    tracing::error!(error = %error, "MCP OAuth: failed to {context}");
    no_store(
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "error": "temporarily_unavailable",
                "error_description": "the authorization server could not complete the request; retry shortly",
            })),
        )
            .into_response(),
    )
}

/// RFC 6749 §2.3.1: Basic credentials are form-urlencoded before base64.
fn basic_credentials(headers: &HeaderMap) -> Option<(String, String)> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, encoded) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("basic") {
        return None;
    }
    let decoded = String::from_utf8(STANDARD.decode(encoded.trim()).ok()?).ok()?;
    let (id, secret) = decoded.split_once(':')?;
    let unescape = |s: &str| {
        url::form_urlencoded::parse(format!("v={s}").as_bytes())
            .next()
            .map(|(_, v)| v.into_owned())
    };
    Some((unescape(id)?, unescape(secret)?))
}

/// Authenticate the client as registered. Confidential clients may present
/// their secret by Basic or in the body; public clients present none.
async fn authenticate_client(
    state: &McpOAuthState,
    headers: &HeaderMap,
    request: &TokenRequest,
) -> Result<String, Response> {
    let basic = basic_credentials(headers);
    let used_basic = basic.is_some();
    let (client_id, secret) = match (basic, &request.client_id) {
        (Some((id, _)), Some(body_id)) if &id != body_id => {
            return Err(invalid_client(
                used_basic,
                "client_id does not match the credentials",
            ));
        }
        (Some((id, secret)), _) => (id, Some(secret)),
        (None, Some(id)) => (id.clone(), request.client_secret.clone()),
        (None, None) => return Err(invalid_client(false, "client authentication is required")),
    };
    let client = match McpOAuth::find_client(&state.pool, &client_id).await {
        Ok(Some(client)) => client,
        Ok(None) => return Err(invalid_client(used_basic, "unknown client")),
        Err(error) => return Err(server_error("load OAuth client", error)),
    };
    let authenticated = match (&client.client_secret_hash, secret.as_deref()) {
        (None, None) => true,
        (Some(hash), Some(secret)) => digest_matches(secret, hash),
        _ => false,
    };
    if !authenticated {
        return Err(invalid_client(used_basic, "client authentication failed"));
    }
    Ok(client.client_id)
}

fn invalid_client(used_basic: bool, description: &str) -> Response {
    let mut response = token_error(StatusCode::UNAUTHORIZED, "invalid_client", description);
    if used_basic {
        response.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            HeaderValue::from_static("Basic realm=\"vibe-kanban\""),
        );
    }
    response
}

struct IssuedTokens {
    access_token: String,
    refresh_token: String,
}

impl IssuedTokens {
    fn mint() -> Self {
        Self {
            access_token: random_token(),
            refresh_token: random_token(),
        }
    }

    fn hashes(&self) -> (Vec<u8>, Vec<u8>) {
        (digest(&self.access_token), digest(&self.refresh_token))
    }

    fn response(self) -> Response {
        no_store(
            Json(json!({
                "access_token": self.access_token,
                "token_type": "Bearer",
                "expires_in": ACCESS_TTL.num_seconds(),
                "refresh_token": self.refresh_token,
                "scope": SCOPE,
            }))
            .into_response(),
        )
    }
}

fn token_hashes<'a>(
    access: &'a [u8],
    refresh: &'a [u8],
    now: DateTime<Utc>,
) -> IssuedTokenHashes<'a> {
    IssuedTokenHashes {
        access_hash: access,
        access_expires_at: now + ACCESS_TTL,
        refresh_hash: refresh,
        refresh_expires_at: now + REFRESH_TTL,
    }
}

async fn token(
    State(state): State<McpOAuthState>,
    headers: HeaderMap,
    form: Result<Form<TokenRequest>, FormRejection>,
) -> Response {
    let Some(config) = state.enabled() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(Form(request)) = form else {
        return token_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "the body must be application/x-www-form-urlencoded token parameters",
        );
    };
    let client_id = match authenticate_client(&state, &headers, &request).await {
        Ok(client_id) => client_id,
        Err(response) => return response,
    };
    if request
        .resource
        .as_deref()
        .is_some_and(|r| r != config.resource())
    {
        return token_error(
            StatusCode::BAD_REQUEST,
            "invalid_target",
            "resource must be this server's MCP endpoint",
        );
    }
    if !scope_is_supported(request.scope.as_deref()) {
        return token_error(
            StatusCode::BAD_REQUEST,
            "invalid_scope",
            "only the mcp scope is supported",
        );
    }
    match request.grant_type.as_deref() {
        Some("authorization_code") => exchange_code(&state, &client_id, &request).await,
        Some("refresh_token") => refresh(&state, &client_id, &request).await,
        _ => token_error(
            StatusCode::BAD_REQUEST,
            "unsupported_grant_type",
            "grant_type must be authorization_code or refresh_token",
        ),
    }
}

async fn exchange_code(state: &McpOAuthState, client_id: &str, request: &TokenRequest) -> Response {
    let (Some(code), Some(redirect_uri), Some(verifier)) = (
        request.code.as_deref(),
        request.redirect_uri.as_deref(),
        request.code_verifier.as_deref(),
    ) else {
        return token_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "code, redirect_uri and code_verifier are required",
        );
    };
    let now = Utc::now();
    let authorization = match McpOAuth::find_authorization_by_code(&state.pool, &digest(code)).await
    {
        Ok(Some(authorization)) => authorization,
        Ok(None) => return invalid_grant("the authorization code is invalid"),
        Err(error) => return server_error("load authorization code", error),
    };
    // Check the code's bindings first, so a caller holding a leaked code
    // without this client's credentials, redirect URI and PKCE verifier can
    // neither redeem it nor trigger replay revocation of the real grant.
    if authorization.client_id != client_id
        || authorization.redirect_uri != redirect_uri
        || !pkce_matches(verifier, &authorization.code_challenge)
    {
        return invalid_grant(
            "the authorization code does not match this client, redirect_uri or code_verifier",
        );
    }
    if authorization.status == "exchanged" {
        return code_replayed(state, &authorization.id, now).await;
    }
    if authorization.status != "approved" || authorization.expires_at <= timestamp(now) {
        return invalid_grant("the authorization code has expired");
    }

    let tokens = IssuedTokens::mint();
    let (access_hash, refresh_hash) = tokens.hashes();
    let grant_id = Uuid::new_v4().to_string();
    match McpOAuth::exchange_code(
        &state.pool,
        &authorization,
        &grant_id,
        token_hashes(&access_hash, &refresh_hash, now),
        now,
    )
    .await
    {
        Ok(ExchangeOutcome::Issued { .. }) => {
            tracing::info!(client_id = %client_id, grant_id = %grant_id, "Issued MCP OAuth grant");
            tokens.response()
        }
        // Lost a race with another exchange of the same code: that is a
        // replay too, so the winner's grant is revoked as well.
        Ok(ExchangeOutcome::NotApproved) => code_replayed(state, &authorization.id, now).await,
        Err(error) => server_error("issue tokens", error),
    }
}

/// A replayed code means it leaked: revoke everything it minted.
async fn code_replayed(
    state: &McpOAuthState,
    authorization_id: &str,
    now: DateTime<Utc>,
) -> Response {
    match McpOAuth::revoke_exchanged_grant(&state.pool, authorization_id, now).await {
        Ok(revoked) => {
            if revoked {
                tracing::warn!("MCP OAuth authorization code replayed; grant revoked");
            }
            invalid_grant("the authorization code was already used")
        }
        Err(error) => server_error("revoke replayed grant", error),
    }
}

async fn refresh(state: &McpOAuthState, client_id: &str, request: &TokenRequest) -> Response {
    let Some(refresh_token) = request.refresh_token.as_deref() else {
        return token_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "refresh_token is required",
        );
    };
    let now = Utc::now();
    let refresh_hash = digest(refresh_token);
    let record = match McpOAuth::find_token(&state.pool, &refresh_hash).await {
        Ok(Some(record)) if record.kind == "refresh" && record.client_id == client_id => record,
        Ok(_) => return invalid_grant("the refresh token is invalid"),
        Err(error) => return server_error("load refresh token", error),
    };
    let revoke_for_reuse = || async {
        match McpOAuth::revoke_grant(&state.pool, &record.grant_id, now).await {
            Ok(_) => {
                tracing::warn!(client_id = %client_id, grant_id = %record.grant_id, "MCP OAuth refresh token reused; grant revoked");
                invalid_grant("the refresh token was already used; reconnect the application")
            }
            Err(error) => server_error("revoke reused grant", error),
        }
    };
    if record.revoked_at.is_some() {
        return revoke_for_reuse().await;
    }
    if record.grant_revoked_at.is_some() || record.expires_at <= timestamp(now) {
        return invalid_grant(
            "the refresh token has expired or was revoked; reconnect the application",
        );
    }

    let tokens = IssuedTokens::mint();
    let (access_hash, new_refresh_hash) = tokens.hashes();
    match McpOAuth::rotate_refresh(
        &state.pool,
        &refresh_hash,
        &record.grant_id,
        token_hashes(&access_hash, &new_refresh_hash, now),
        now,
    )
    .await
    {
        Ok(RotateOutcome::Rotated) => tokens.response(),
        Ok(RotateOutcome::AlreadyRevoked) => revoke_for_reuse().await,
        Err(error) => server_error("rotate refresh token", error),
    }
}

// ---------------------------------------------------------------------------
// Resource-server check (reverse-proxy forward_auth) and grant management

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    scheme
        .eq_ignore_ascii_case("bearer")
        .then(|| token.trim())
        .filter(|token| !token.is_empty())
}

fn unauthorized(config: &McpOAuthConfig, presented: bool) -> Response {
    let mut challenge = format!(
        "Bearer resource_metadata=\"{}\"",
        config.resource_metadata_url()
    );
    let message = if presented {
        challenge.push_str(", error=\"invalid_token\"");
        "Unauthorized: the access token is invalid, expired or revoked"
    } else {
        "Unauthorized: an OAuth access token is required"
    };
    let mut response = (
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "jsonrpc": "2.0",
            "id": null,
            "error": { "code": -32001, "message": message },
        })),
    )
        .into_response();
    if let Ok(value) = HeaderValue::from_str(&challenge) {
        response
            .headers_mut()
            .insert(header::WWW_AUTHENTICATE, value);
    }
    no_store(response)
}

async fn verify(State(state): State<McpOAuthState>, headers: HeaderMap) -> Response {
    let Some(config) = state.enabled() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(token) = bearer_token(&headers) else {
        return unauthorized(config, false);
    };
    let now = Utc::now();
    match McpOAuth::verify_access_token(&state.pool, &digest(token), &config.resource(), now).await
    {
        Ok(Some(record)) => {
            if let Err(error) =
                McpOAuth::touch_grant(&state.pool, &record.grant_id, now, TOUCH_INTERVAL).await
            {
                tracing::warn!(error = %error, "MCP OAuth: failed to record grant use");
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(None) => unauthorized(config, true),
        Err(error) => server_error("verify access token", error),
    }
}

async fn list_grants(State(state): State<McpOAuthState>) -> Response {
    if state.enabled().is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    match McpOAuth::list_live_grants(&state.pool).await {
        Ok(grants) => {
            Json(ApiResponse::<Vec<McpOAuthGrantSummary>>::success(grants)).into_response()
        }
        Err(error) => server_error("list grants", error),
    }
}

async fn revoke_grant(State(state): State<McpOAuthState>, Path(id): Path<String>) -> Response {
    if state.enabled().is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    match McpOAuth::revoke_grant(&state.pool, &id, Utc::now()).await {
        Ok(true) => {
            tracing::info!(grant_id = %id, "Revoked MCP OAuth grant");
            Json(ApiResponse::<()>::success(())).into_response()
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()>::error("No live grant with that id")),
        )
            .into_response(),
        Err(error) => server_error("revoke grant", error),
    }
}
