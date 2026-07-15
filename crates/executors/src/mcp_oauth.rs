//! Minimal OAuth 2.1 client plumbing for authenticating MCP servers.
//!
//! Implements the client side of the MCP authorization spec (2025-06-18):
//! protected-resource metadata discovery (RFC 9728) starting from a 401's
//! `WWW-Authenticate` header with the spec's well-known fallbacks,
//! authorization-server metadata (RFC 8414 / OIDC discovery), dynamic client
//! registration (RFC 7591), and the authorization-code + PKCE (RFC 7636)
//! exchange with the `resource` indicator (RFC 8707).
//!
//! Hand-rolled over crates already in the workspace (`reqwest`, `sha2`,
//! `rand`, `base64`) for the same reason the probe in `mcp_test` is: the
//! OAuth-crate ecosystem doesn't cover the MCP-specific discovery steps, and
//! the parts it would cover are small. Pure logic (header parsing, well-known
//! URL construction, PKCE, URL assembly) is kept in standalone functions so it
//! unit-tests without I/O. Access tokens are returned to the caller and never
//! logged here.

use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::Duration,
};

use base64::Engine;
use rand::{Rng, distributions::Alphanumeric};
use reqwest::{Url, header::WWW_AUTHENTICATE, redirect::Policy};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// Everything discovery learns that the authorize/exchange steps need.
#[derive(Debug, Clone)]
pub struct AuthServerMetadata {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub registration_endpoint: Option<String>,
    pub revocation_endpoint: Option<String>,
    pub scopes_supported: Vec<String>,
    /// Canonical resource URI to bind tokens to (RFC 8707): the protected
    /// resource metadata's `resource` when present, else the MCP server URL.
    pub resource: String,
}

/// A PKCE verifier/challenge pair (S256).
#[derive(Debug, Clone)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

/// HTTP policy shared by discovery, registration, and token exchange.
///
/// Every request resolves and validates its destination, then pins those DNS
/// answers into the reqwest client that makes the connection. Redirects are
/// disabled so a validated public URL cannot bounce to an internal address.
#[derive(Clone)]
pub struct OAuthHttpClient {
    timeout: Duration,
    loopback_dev_origin: Option<Origin>,
    mcp_origin: Origin,
    cloudflare_access: Option<(Origin, String, String)>,
}

impl std::fmt::Debug for OAuthHttpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthHttpClient")
            .field("timeout", &self.timeout)
            .field("loopback_dev_origin", &self.loopback_dev_origin)
            .field(
                "cloudflare_access",
                &self.cloudflare_access.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Origin {
    scheme: String,
    host: String,
    port: u16,
}

impl Origin {
    fn from_url(url: &Url) -> Result<Self, String> {
        let host = url
            .host_str()
            .ok_or_else(|| "OAuth URL has no host".to_string())?;
        let port = url
            .port_or_known_default()
            .ok_or_else(|| "OAuth URL has no usable port".to_string())?;
        Ok(Self {
            scheme: url.scheme().to_ascii_lowercase(),
            host: host.trim_end_matches('.').to_ascii_lowercase(),
            port,
        })
    }
}

impl OAuthHttpClient {
    pub fn new(timeout: Duration, mcp_server_url: &str) -> Result<Self, String> {
        let mcp_url = Url::parse(mcp_server_url)
            .map_err(|_| "configured MCP server URL is invalid".to_string())?;
        let origin = Origin::from_url(&mcp_url)?;
        let loopback_dev_origin = (origin.scheme == "http"
            && host_is_literal_loopback(&origin.host))
        .then_some(origin.clone());
        Ok(Self {
            timeout,
            loopback_dev_origin,
            mcp_origin: origin,
            cloudflare_access: None,
        })
    }

    pub fn with_cloudflare_access(mut self, client_id: String, client_secret: String) -> Self {
        self.cloudflare_access = Some((self.mcp_origin.clone(), client_id, client_secret));
        self
    }

    async fn client_for(&self, raw_url: &str) -> Result<(reqwest::Client, Url), String> {
        let mut url =
            Url::parse(raw_url).map_err(|_| "OAuth endpoint URL is invalid".to_string())?;
        if !url.username().is_empty() || url.password().is_some() {
            return Err("OAuth endpoint URL must not contain credentials".to_string());
        }
        let origin = Origin::from_url(&url)?;
        canonicalize_request_host(&mut url, &origin)?;
        let loopback_dev = self.loopback_dev_origin.as_ref() == Some(&origin);
        if origin.scheme != "https" && !(origin.scheme == "http" && loopback_dev) {
            return Err("OAuth endpoint must use HTTPS".to_string());
        }

        let port = origin.port;
        let addresses = tokio::time::timeout(self.timeout, resolve_host(&origin.host, port))
            .await
            .map_err(|_| "OAuth endpoint DNS resolution timed out".to_string())??;
        if addresses.is_empty() {
            return Err("OAuth endpoint host resolved to no addresses".to_string());
        }
        if !loopback_dev && addresses.iter().any(|addr| !is_public_ip(addr.ip())) {
            return Err("OAuth endpoint resolves to a non-public address".to_string());
        }
        if loopback_dev && addresses.iter().any(|addr| !addr.ip().is_loopback()) {
            return Err("loopback OAuth endpoint resolved outside loopback".to_string());
        }

        let mut builder = reqwest::Client::builder()
            .timeout(self.timeout)
            // Environment/system proxies would resolve and connect on our
            // behalf, bypassing the validated address pinning below.
            .no_proxy()
            .redirect(Policy::none());
        // Pin the exact answers that passed validation, closing the DNS
        // rebinding window between policy evaluation and connection setup.
        builder = builder.resolve_to_addrs(&origin.host, &addresses);
        let client = builder
            .build()
            .map_err(|_| "could not initialize OAuth HTTP client".to_string())?;
        Ok((client, url))
    }

    async fn get(&self, url: &str) -> Result<reqwest::Response, String> {
        let (client, parsed) = self.client_for(url).await?;
        let mut request = client.get(parsed.clone());
        if let Some((access_origin, id, secret)) = &self.cloudflare_access
            && Origin::from_url(&parsed).ok().as_ref() == Some(access_origin)
        {
            request = request
                .header("CF-Access-Client-Id", id)
                .header("CF-Access-Client-Secret", secret);
        }
        request
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|e| format!("OAuth request failed: {e}"))
    }

    async fn post_json(&self, url: &str, body: &Value) -> Result<reqwest::Response, String> {
        let (client, parsed) = self.client_for(url).await?;
        let mut request = client.post(parsed.clone());
        if let Some((access_origin, id, secret)) = &self.cloudflare_access
            && Origin::from_url(&parsed).ok().as_ref() == Some(access_origin)
        {
            request = request
                .header("CF-Access-Client-Id", id)
                .header("CF-Access-Client-Secret", secret);
        }
        request
            .json(body)
            .send()
            .await
            .map_err(|e| format!("OAuth request failed: {e}"))
    }

    async fn post_form(&self, url: &str, body: String) -> Result<reqwest::Response, String> {
        let (client, parsed) = self.client_for(url).await?;
        let mut request = client.post(parsed.clone());
        if let Some((access_origin, id, secret)) = &self.cloudflare_access
            && Origin::from_url(&parsed).ok().as_ref() == Some(access_origin)
        {
            request = request
                .header("CF-Access-Client-Id", id)
                .header("CF-Access-Client-Secret", secret);
        }
        request
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(body)
            .send()
            .await
            .map_err(|e| format!("OAuth request failed: {e}"))
    }
}

fn canonicalize_request_host(url: &mut Url, origin: &Origin) -> Result<(), String> {
    if url.host_str().is_some_and(|host| host.ends_with('.')) {
        // reqwest resolver overrides are keyed by hostname. Ensure the URL
        // uses the same normalized key that was validated and pinned.
        url.set_host(Some(&origin.host))
            .map_err(|_| "OAuth endpoint host is invalid".to_string())?;
    }
    Ok(())
}

async fn resolve_host(host: &str, port: u16) -> Result<Vec<SocketAddr>, String> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(vec![SocketAddr::new(ip, port)]);
    }
    tokio::net::lookup_host((host, port))
        .await
        .map(|iter| iter.collect())
        .map_err(|_| "OAuth endpoint host could not be resolved".to_string())
}

fn host_is_literal_loopback(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(a == 0
        || a == 10
        || a == 127
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 0 && c == 2)
        || (a == 192 && b == 168)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || a >= 224)
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_public_ipv4(v4);
    }
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_multicast()
        || (segments[0] == 0x0064 && segments[1] == 0xff9b && segments[2] == 0) // NAT64 64:ff9b::/96
        || (segments[0] == 0x0064 && segments[1] == 0xff9b && segments[2] == 1) // NAT64 local-use /48
        || segments[0] == 0x2002 // 6to4 2002::/16
        || (segments[0] == 0x2001 && segments[1] == 0) // Teredo 2001::/32
        || (segments[0] & 0xfe00) == 0xfc00 // unique local fc00::/7
        || (segments[0] & 0xffc0) == 0xfe80 // link local fe80::/10
        || (segments[0] == 0x2001 && segments[1] == 0x0db8)) // documentation
}

impl Pkce {
    pub fn generate() -> Self {
        let verifier: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();
        let challenge = s256_challenge(&verifier);
        Self {
            verifier,
            challenge,
        }
    }
}

/// S256 code challenge: BASE64URL-without-padding(SHA256(verifier)).
fn s256_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

/// Random URL-safe `state` value for CSRF binding.
pub fn generate_state() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

/// Extract the `resource_metadata="…"` URL from a `WWW-Authenticate` header
/// value (RFC 9728 §5.1).
pub fn parse_resource_metadata_url(www_authenticate: &str) -> Option<String> {
    let lower = www_authenticate.to_ascii_lowercase();
    let key = "resource_metadata=";
    let start = lower.find(key)? + key.len();
    let rest = &www_authenticate[start..];
    let rest = rest.strip_prefix('"').unwrap_or(rest);
    let end = rest.find('"').unwrap_or_else(|| {
        rest.find([',', ' ']).unwrap_or(rest.len()) // unquoted fallback
    });
    let url = rest[..end].trim();
    (!url.is_empty()).then(|| url.to_string())
}

/// Well-known URL candidates for `suffix` relative to `base`, most specific
/// first: RFC 8414-style path insertion, then the origin root.
fn well_known_candidates(base: &Url, suffix: &str) -> Vec<String> {
    let origin = format!(
        "{}://{}",
        base.scheme(),
        base.host_str().map_or_else(String::new, |h| {
            match base.port() {
                Some(p) => format!("{h}:{p}"),
                None => h.to_string(),
            }
        })
    );
    let path = base.path().trim_end_matches('/');
    let mut candidates = Vec::new();
    if !path.is_empty() {
        candidates.push(format!("{origin}/.well-known/{suffix}{path}"));
    }
    candidates.push(format!("{origin}/.well-known/{suffix}"));
    candidates
}

/// Metadata URL candidates for an authorization-server issuer: RFC 8414
/// path-insertion forms first, then OIDC issuer-suffix discovery (how
/// path-based issuers like Keycloak realms actually publish it — RFC 8414
/// insertion would miss them), then OIDC insertion forms, deduplicated.
fn as_metadata_candidates(as_url: &Url) -> Vec<String> {
    let mut candidates = well_known_candidates(as_url, "oauth-authorization-server");
    let issuer = as_url.as_str().trim_end_matches('/');
    candidates.push(format!("{issuer}/.well-known/openid-configuration"));
    for candidate in well_known_candidates(as_url, "openid-configuration") {
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }
    candidates
}

/// GET a JSON document, treating any non-success status or parse failure as
/// a soft error (the caller tries the next candidate URL).
async fn fetch_json(client: &OAuthHttpClient, url: &str) -> Result<Value, String> {
    let resp = client.get(url).await?;
    if resp.status().is_redirection() {
        return Err(
            "OAuth metadata was redirected to an interactive login (HTTP 3xx). If this server is protected by Cloudflare Access, configure a Cloudflare Access service token for shared gateway authentication or adjust the Access policy"
                .to_string(),
        );
    }
    if !resp.status().is_success() {
        return Err(format!(
            "OAuth endpoint returned HTTP {}",
            resp.status().as_u16()
        ));
    }
    resp.json::<Value>()
        .await
        .map_err(|_| "OAuth endpoint returned invalid JSON".to_string())
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(String::from)
}

fn string_array(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

/// Discover the authorization server for an MCP server URL.
///
/// `www_authenticate_hint` is the header captured by an earlier probe, if
/// any; when it names a `resource_metadata` URL the extra 401-eliciting
/// request is skipped.
pub async fn discover(
    client: &OAuthHttpClient,
    mcp_url: &str,
    www_authenticate_hint: Option<&str>,
) -> Result<AuthServerMetadata, String> {
    let base = Url::parse(mcp_url).map_err(|e| format!("invalid server url `{mcp_url}`: {e}"))?;

    // 1. Find the protected-resource metadata URL: header hint, fresh 401,
    //    then the spec's well-known fallbacks.
    let mut prm_candidates: Vec<String> = Vec::new();
    if let Some(url) = www_authenticate_hint.and_then(parse_resource_metadata_url) {
        prm_candidates.push(url);
    } else if let Ok(resp) = client.get(mcp_url).await
        && let Some(header) = resp
            .headers()
            .get(WWW_AUTHENTICATE)
            .and_then(|v| v.to_str().ok())
        && let Some(url) = parse_resource_metadata_url(header)
    {
        prm_candidates.push(url);
    }
    prm_candidates.extend(well_known_candidates(&base, "oauth-protected-resource"));

    // 2. Fetch protected-resource metadata; fall back to treating the MCP
    //    server's own origin as the authorization server (pre-RFC 9728
    //    servers) when none of the candidates resolve.
    let mut resource = canonical_resource(&base);
    let mut scopes_supported = Vec::new();
    let mut auth_server = format!(
        "{}://{}",
        base.scheme(),
        base.host_str()
            .map(|h| match base.port() {
                Some(p) => format!("{h}:{p}"),
                None => h.to_string(),
            })
            .ok_or_else(|| format!("server url `{mcp_url}` has no host"))?
    );
    let mut prm_errors = Vec::new();
    for candidate in &prm_candidates {
        match fetch_json(client, candidate).await {
            Ok(prm) => {
                if let Some(first) = string_array(&prm, "authorization_servers")
                    .into_iter()
                    .next()
                {
                    auth_server = first;
                }
                if let Some(res) = string_field(&prm, "resource") {
                    resource = res;
                }
                scopes_supported = string_array(&prm, "scopes_supported");
                prm_errors.clear();
                break;
            }
            Err(e) => prm_errors.push(e),
        }
    }

    // 3. Fetch authorization-server metadata (RFC 8414, then OIDC discovery).
    let as_url = Url::parse(auth_server.trim_end_matches('/'))
        .map_err(|e| format!("invalid authorization server url `{auth_server}`: {e}"))?;
    let as_candidates = as_metadata_candidates(&as_url);

    let mut as_errors = Vec::new();
    for candidate in &as_candidates {
        match fetch_json(client, candidate).await {
            Ok(meta) => {
                let authorization_endpoint = string_field(&meta, "authorization_endpoint");
                let token_endpoint = string_field(&meta, "token_endpoint");
                if let (Some(authorization_endpoint), Some(token_endpoint)) =
                    (authorization_endpoint, token_endpoint)
                {
                    validate_server_metadata(&as_url, &meta)?;
                    return Ok(AuthServerMetadata {
                        authorization_endpoint,
                        token_endpoint,
                        registration_endpoint: string_field(&meta, "registration_endpoint"),
                        revocation_endpoint: string_field(&meta, "revocation_endpoint"),
                        scopes_supported,
                        resource,
                    });
                }
                as_errors.push(format!("{candidate}: metadata lacks endpoints"));
            }
            Err(e) => as_errors.push(e),
        }
    }

    Err(format!(
        "could not discover the OAuth authorization server for {mcp_url}: {}",
        as_errors
            .last()
            .or(prm_errors.last())
            .cloned()
            .unwrap_or_else(|| "no metadata endpoints responded".to_string())
    ))
}

fn same_origin(left: &Url, right: &Url) -> bool {
    Origin::from_url(left).ok() == Origin::from_url(right).ok()
}

fn validate_server_metadata(expected_issuer: &Url, metadata: &Value) -> Result<(), String> {
    let expected_origin = Origin::from_url(expected_issuer)?;
    let loopback_dev =
        expected_origin.scheme == "http" && host_is_literal_loopback(&expected_origin.host);
    if expected_issuer.scheme() != "https" && !loopback_dev {
        return Err("authorization server issuer must use HTTPS".to_string());
    }
    if let Some(issuer) = string_field(metadata, "issuer") {
        let issuer = Url::parse(&issuer)
            .map_err(|_| "authorization server metadata has an invalid issuer".to_string())?;
        if !same_origin(expected_issuer, &issuer) {
            return Err("authorization server metadata issuer has a different origin".to_string());
        }
    }
    for field in [
        "authorization_endpoint",
        "token_endpoint",
        "registration_endpoint",
    ] {
        let Some(raw) = string_field(metadata, field) else {
            continue;
        };
        let endpoint =
            Url::parse(&raw).map_err(|_| format!("authorization server {field} is invalid"))?;
        let requires_issuer_origin = field != "authorization_endpoint";
        let secure_endpoint = endpoint.scheme() == "https"
            || (loopback_dev && same_origin(expected_issuer, &endpoint));
        if !secure_endpoint || (requires_issuer_origin && !same_origin(expected_issuer, &endpoint))
        {
            return Err(format!(
                "authorization server {field} must use HTTPS{}",
                if requires_issuer_origin {
                    " and the issuer origin"
                } else {
                    ""
                }
            ));
        }
    }
    Ok(())
}

/// Canonical resource URI: the server URL without query or fragment.
fn canonical_resource(url: &Url) -> String {
    let mut canonical = url.clone();
    canonical.set_query(None);
    canonical.set_fragment(None);
    canonical.to_string()
}

/// Dynamic client registration (RFC 7591) as a public client. Returns the
/// issued `client_id`.
pub async fn register_client(
    client: &OAuthHttpClient,
    registration_endpoint: &str,
    redirect_uri: &str,
) -> Result<String, String> {
    let body = json!({
        "client_name": "Vibe Kanban",
        "redirect_uris": [redirect_uri],
        "grant_types": ["authorization_code"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none",
    });
    let resp = client.post_json(registration_endpoint, &body).await?;
    let status = resp.status();
    let value: Value = resp
        .json()
        .await
        .map_err(|_| "client registration returned invalid JSON".to_string())?;
    if !status.is_success() {
        return Err(format!(
            "client registration failed (HTTP {})",
            status.as_u16()
        ));
    }
    string_field(&value, "client_id")
        .ok_or_else(|| "client registration response lacks client_id".to_string())
}

/// Assemble the authorization URL for the consent redirect.
pub fn build_authorize_url(
    authorization_endpoint: &str,
    client_id: &str,
    redirect_uri: &str,
    code_challenge: &str,
    state: &str,
    resource: &str,
    scopes: &[String],
) -> Result<String, String> {
    let mut url = Url::parse(authorization_endpoint)
        .map_err(|e| format!("invalid authorization endpoint: {e}"))?;
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("response_type", "code")
            .append_pair("client_id", client_id)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("code_challenge", code_challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", state)
            .append_pair("resource", resource);
        if !scopes.is_empty() {
            query.append_pair("scope", &scopes.join(" "));
        }
    }
    Ok(url.to_string())
}

#[derive(Clone, Serialize, Deserialize)]
pub struct OAuthTokenSet {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: Option<String>,
    pub expires_in: Option<u64>,
    pub scope: Option<String>,
}

impl std::fmt::Debug for OAuthTokenSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthTokenSet")
            .field("access_token", &"[REDACTED]")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("token_type", &self.token_type)
            .field("expires_in", &self.expires_in)
            .field("scope", &self.scope)
            .finish()
    }
}

/// Exchange an authorization code while retaining refresh/expiry metadata for
/// the shared gateway. The token set's Debug implementation is redacted.
pub async fn exchange_token_set(
    client: &OAuthHttpClient,
    token_endpoint: &str,
    client_id: &str,
    code: &str,
    pkce_verifier: &str,
    redirect_uri: &str,
    resource: &str,
) -> Result<OAuthTokenSet, String> {
    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", client_id),
        ("code_verifier", pkce_verifier),
        ("resource", resource),
    ];
    // This reqwest build has no `.form()` helper; `Url`'s query serializer
    // produces the same application/x-www-form-urlencoded encoding.
    let mut encoder = Url::parse("http://encode.invalid/").expect("static url parses");
    encoder.query_pairs_mut().extend_pairs(params);
    let body = encoder.query().unwrap_or_default().to_string();
    let resp = client.post_form(token_endpoint, body).await?;
    let status = resp.status();
    let value: Value = resp
        .json()
        .await
        .map_err(|_| "token endpoint returned invalid JSON".to_string())?;
    if !status.is_success() {
        return Err(format!("token exchange failed (HTTP {})", status.as_u16()));
    }
    let access_token = string_field(&value, "access_token")
        .ok_or_else(|| "token response lacks access_token".to_string())?;
    Ok(OAuthTokenSet {
        access_token,
        refresh_token: string_field(&value, "refresh_token"),
        token_type: string_field(&value, "token_type"),
        expires_in: value.get("expires_in").and_then(Value::as_u64),
        scope: string_field(&value, "scope"),
    })
}

pub async fn exchange_code(
    client: &OAuthHttpClient,
    token_endpoint: &str,
    client_id: &str,
    code: &str,
    pkce_verifier: &str,
    redirect_uri: &str,
    resource: &str,
) -> Result<String, String> {
    exchange_token_set(
        client,
        token_endpoint,
        client_id,
        code,
        pkce_verifier,
        redirect_uri,
        resource,
    )
    .await
    .map(|tokens| tokens.access_token)
}

pub async fn refresh_access_token(
    client: &OAuthHttpClient,
    token_endpoint: &str,
    client_id: &str,
    client_secret: Option<&str>,
    refresh_token: &str,
    resource: &str,
) -> Result<OAuthTokenSet, String> {
    let mut params = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", client_id),
        ("resource", resource),
    ];
    if let Some(secret) = client_secret {
        params.push(("client_secret", secret));
    }
    let mut encoder = Url::parse("http://encode.invalid/").expect("static url parses");
    encoder.query_pairs_mut().extend_pairs(params);
    let response = client
        .post_form(
            token_endpoint,
            encoder.query().unwrap_or_default().to_string(),
        )
        .await?;
    let status = response.status();
    let value: Value = response
        .json()
        .await
        .map_err(|_| "token endpoint returned invalid JSON".to_string())?;
    if !status.is_success() {
        return Err(format!("token refresh failed (HTTP {})", status.as_u16()));
    }
    Ok(OAuthTokenSet {
        access_token: string_field(&value, "access_token")
            .ok_or_else(|| "token response lacks access_token".to_string())?,
        refresh_token: string_field(&value, "refresh_token")
            .or_else(|| Some(refresh_token.to_string())),
        token_type: string_field(&value, "token_type"),
        expires_in: value.get("expires_in").and_then(Value::as_u64),
        scope: string_field(&value, "scope"),
    })
}

pub async fn revoke_token(
    client: &OAuthHttpClient,
    revocation_endpoint: &str,
    client_id: Option<&str>,
    client_secret: Option<&str>,
    token: &str,
    token_type_hint: &str,
) -> Result<(), String> {
    let mut params = vec![("token", token), ("token_type_hint", token_type_hint)];
    if let Some(client_id) = client_id {
        params.push(("client_id", client_id));
    }
    if let Some(client_secret) = client_secret {
        params.push(("client_secret", client_secret));
    }
    let mut encoder = Url::parse("http://encode.invalid/").expect("static url parses");
    encoder.query_pairs_mut().extend_pairs(params);
    let response = client
        .post_form(
            revocation_endpoint,
            encoder.query().unwrap_or_default().to_string(),
        )
        .await?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!(
            "token revocation failed (HTTP {})",
            response.status().as_u16()
        ))
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::AsyncWriteExt;

    use super::*;

    #[test]
    fn parses_resource_metadata_from_header() {
        let header = r#"Bearer realm="mcp", resource_metadata="https://x.dev/.well-known/oauth-protected-resource/_mcp""#;
        assert_eq!(
            parse_resource_metadata_url(header).as_deref(),
            Some("https://x.dev/.well-known/oauth-protected-resource/_mcp")
        );
        // Unquoted value, trailing parameter.
        let unquoted = "Bearer resource_metadata=https://x.dev/prm, error=invalid_token";
        assert_eq!(
            parse_resource_metadata_url(unquoted).as_deref(),
            Some("https://x.dev/prm")
        );
        assert_eq!(parse_resource_metadata_url("Bearer realm=\"x\""), None);
    }

    #[test]
    fn well_known_candidates_insert_path() {
        let base = Url::parse("https://claude.sweetgreen.dev/_mcp/_sgsc/mcp").unwrap();
        assert_eq!(
            well_known_candidates(&base, "oauth-protected-resource"),
            vec![
                "https://claude.sweetgreen.dev/.well-known/oauth-protected-resource/_mcp/_sgsc/mcp"
                    .to_string(),
                "https://claude.sweetgreen.dev/.well-known/oauth-protected-resource".to_string(),
            ]
        );
        let root = Url::parse("https://as.example.com/").unwrap();
        assert_eq!(
            well_known_candidates(&root, "openid-configuration"),
            vec!["https://as.example.com/.well-known/openid-configuration".to_string()]
        );
        let with_port = Url::parse("http://127.0.0.1:3334/mcp").unwrap();
        assert_eq!(
            well_known_candidates(&with_port, "oauth-authorization-server"),
            vec![
                "http://127.0.0.1:3334/.well-known/oauth-authorization-server/mcp".to_string(),
                "http://127.0.0.1:3334/.well-known/oauth-authorization-server".to_string(),
            ]
        );
    }

    #[test]
    fn as_metadata_candidates_cover_path_based_oidc_issuers() {
        // Keycloak-style issuer: OIDC discovery is appended to the issuer
        // path, which the RFC 8414 insertion form does not produce.
        let keycloak = Url::parse("https://kc.example.com/realms/myrealm").unwrap();
        let candidates = as_metadata_candidates(&keycloak);
        assert!(candidates.contains(
            &"https://kc.example.com/realms/myrealm/.well-known/openid-configuration".to_string()
        ));
        assert!(
            candidates.contains(
                &"https://kc.example.com/.well-known/oauth-authorization-server/realms/myrealm"
                    .to_string()
            )
        );
        // RFC 8414 forms are preferred (tried first).
        assert!(candidates[0].contains("oauth-authorization-server"));

        // Root issuer: appended and insertion OIDC forms coincide — no dupes.
        let root = Url::parse("https://as.example.com").unwrap();
        let candidates = as_metadata_candidates(&root);
        assert_eq!(
            candidates
                .iter()
                .filter(|c| c.contains("openid-configuration"))
                .count(),
            1
        );
    }

    #[test]
    fn pkce_challenge_matches_rfc7636_appendix_b() {
        // Verifier and challenge from RFC 7636 appendix B.
        assert_eq!(
            s256_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        let pkce = Pkce::generate();
        assert_eq!(pkce.verifier.len(), 64);
        assert_eq!(pkce.challenge, s256_challenge(&pkce.verifier));
    }

    #[test]
    fn authorize_url_contains_required_params() {
        let url = build_authorize_url(
            "https://as.example.com/authorize?audience=mcp",
            "client-1",
            "http://127.0.0.1:8080/api/mcp-auth/callback",
            "challenge",
            "state-1",
            "https://mcp.example.com/mcp",
            &["mcp.read".to_string(), "mcp.write".to_string()],
        )
        .unwrap();
        let parsed = Url::parse(&url).unwrap();
        let pairs: std::collections::HashMap<_, _> = parsed.query_pairs().collect();
        assert_eq!(pairs["response_type"], "code");
        assert_eq!(pairs["client_id"], "client-1");
        assert_eq!(pairs["code_challenge_method"], "S256");
        assert_eq!(pairs["state"], "state-1");
        assert_eq!(pairs["resource"], "https://mcp.example.com/mcp");
        assert_eq!(pairs["scope"], "mcp.read mcp.write");
        assert_eq!(pairs["audience"], "mcp"); // pre-existing query preserved
    }

    #[test]
    fn canonical_resource_strips_query_and_fragment() {
        let url = Url::parse("https://x.dev/mcp?key=1#frag").unwrap();
        assert_eq!(canonical_resource(&url), "https://x.dev/mcp");
    }

    #[test]
    fn metadata_allows_https_authorization_endpoint_on_another_origin() {
        let issuer = Url::parse("https://issuer.example.com").unwrap();
        let metadata = serde_json::json!({
            "issuer": "https://issuer.example.com",
            "authorization_endpoint": "https://login.example.com/authorize",
            "token_endpoint": "https://issuer.example.com/token",
            "registration_endpoint": "https://issuer.example.com/register"
        });

        validate_server_metadata(&issuer, &metadata).unwrap();
    }

    #[test]
    fn metadata_rejects_cross_origin_credential_endpoints() {
        let issuer = Url::parse("https://issuer.example.com").unwrap();
        for field in ["token_endpoint", "registration_endpoint"] {
            let mut metadata = serde_json::json!({
                "issuer": "https://issuer.example.com",
                "authorization_endpoint": "https://login.example.com/authorize",
                "token_endpoint": "https://issuer.example.com/token",
                "registration_endpoint": "https://issuer.example.com/register"
            });
            metadata[field] = serde_json::json!(format!("https://evil.example/{field}"));

            let error = validate_server_metadata(&issuer, &metadata).unwrap_err();
            assert!(error.contains("issuer origin"), "got: {error}");
        }
    }

    #[test]
    fn loopback_issuer_does_not_allow_external_http_authorization_endpoint() {
        let issuer = Url::parse("http://127.0.0.1:8080").unwrap();
        let metadata = serde_json::json!({
            "issuer": "http://127.0.0.1:8080",
            "authorization_endpoint": "http://login.example.com/authorize",
            "token_endpoint": "http://127.0.0.1:8080/token",
            "registration_endpoint": "http://127.0.0.1:8080/register"
        });

        assert!(validate_server_metadata(&issuer, &metadata).is_err());
    }

    /// Serve `responses` (raw HTTP/1.1 bytes) to sequential connections on a
    /// loopback listener; returns the base URL.
    async fn one_shot_server(responses: Vec<String>) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            for response in responses {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                // Read (and discard) the request head so the client isn't
                // racing a closed socket.
                let mut buf = [0u8; 4096];
                use tokio::io::AsyncReadExt;
                let _ = sock.read(&mut buf).await;
                let _ = sock.write_all(response.as_bytes()).await;
                let _ = sock.shutdown().await;
            }
        });
        format!("http://{addr}")
    }

    fn http_json(status: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    #[tokio::test]
    async fn register_client_parses_client_id() {
        let base = one_shot_server(vec![http_json(
            "201 Created",
            r#"{"client_id":"abc123","token_endpoint_auth_method":"none"}"#,
        )])
        .await;
        let client = OAuthHttpClient::new(Duration::from_secs(2), &base).unwrap();
        let client_id = register_client(&client, &base, "http://127.0.0.1:1/cb")
            .await
            .unwrap();
        assert_eq!(client_id, "abc123");
    }

    #[tokio::test]
    async fn register_client_does_not_surface_remote_error_body() {
        let base = one_shot_server(vec![http_json(
            "400 Bad Request",
            r#"{"error":"invalid_redirect_uri","error_description":"loopback not allowed"}"#,
        )])
        .await;
        let client = OAuthHttpClient::new(Duration::from_secs(2), &base).unwrap();
        let err = register_client(&client, &base, "http://127.0.0.1:1/cb")
            .await
            .unwrap_err();
        assert!(!err.contains("loopback not allowed"), "got: {err}");
        assert!(err.contains("HTTP 400"), "got: {err}");
    }

    #[tokio::test]
    async fn metadata_redirect_has_cloudflare_guidance_without_location() {
        let base = one_shot_server(vec![
            "HTTP/1.1 302 Found\r\nlocation: https://access.example/cdn-cgi/access/login?secret=value\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".to_string(),
        ])
        .await;
        let client = OAuthHttpClient::new(Duration::from_secs(2), &base).unwrap();
        let err = fetch_json(&client, &base).await.unwrap_err();
        assert!(err.contains("Cloudflare Access service token"));
        assert!(!err.contains("secret=value"));
    }

    #[tokio::test]
    async fn exchange_code_returns_access_token_and_errors_loudly() {
        let base = one_shot_server(vec![
            http_json(
                "200 OK",
                r#"{"access_token":"tok-1","token_type":"Bearer"}"#,
            ),
            http_json("400 Bad Request", r#"{"error":"invalid_grant"}"#),
        ])
        .await;
        let client = OAuthHttpClient::new(Duration::from_secs(2), &base).unwrap();
        let token = exchange_code(&client, &base, "c", "code", "v", "http://cb", "res")
            .await
            .unwrap();
        assert_eq!(token, "tok-1");
        let err = exchange_code(&client, &base, "c", "code", "v", "http://cb", "res")
            .await
            .unwrap_err();
        assert!(!err.contains("invalid_grant"), "got: {err}");
        assert!(err.contains("HTTP 400"), "got: {err}");
    }

    #[tokio::test]
    async fn discover_via_prm_well_known_fallback() {
        // One server plays MCP endpoint + PRM + AS metadata across three
        // sequential requests: 401 (no header), PRM at the path-inserted
        // well-known, then AS metadata at the path-inserted well-known.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let origin = format!("http://{addr}");

        let prm = format!(
            r#"{{"resource":"{origin}/mcp","authorization_servers":["{origin}"],"scopes_supported":["mcp"]}}"#
        );
        let as_meta = format!(
            r#"{{"issuer":"{origin}","authorization_endpoint":"{origin}/authorize","token_endpoint":"{origin}/token","registration_endpoint":"{origin}/register"}}"#
        );
        let responses = vec![
            "HTTP/1.1 401 Unauthorized\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                .to_string(),
            http_json("200 OK", &prm),
            http_json("200 OK", &as_meta),
        ];
        tokio::spawn(async move {
            for response in responses {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                let mut buf = [0u8; 4096];
                use tokio::io::AsyncReadExt;
                let _ = sock.read(&mut buf).await;
                let _ = sock.write_all(response.as_bytes()).await;
                let _ = sock.shutdown().await;
            }
        });

        let client = OAuthHttpClient::new(Duration::from_secs(2), &origin).unwrap();
        let meta = discover(&client, &format!("{origin}/mcp"), None)
            .await
            .unwrap();
        assert_eq!(meta.authorization_endpoint, format!("{origin}/authorize"));
        assert_eq!(meta.token_endpoint, format!("{origin}/token"));
        assert_eq!(
            meta.registration_endpoint.as_deref(),
            Some(format!("{origin}/register").as_str())
        );
        assert_eq!(meta.resource, format!("{origin}/mcp"));
        assert_eq!(meta.scopes_supported, vec!["mcp".to_string()]);
    }

    #[tokio::test]
    async fn discover_uses_www_authenticate_hint() {
        // With a hint pointing straight at the PRM URL, discovery skips the
        // 401-eliciting request: PRM then AS metadata only.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let origin = format!("http://{addr}");

        let prm = format!(r#"{{"authorization_servers":["{origin}"]}}"#);
        let as_meta =
            format!(r#"{{"authorization_endpoint":"{origin}/a","token_endpoint":"{origin}/t"}}"#);
        let responses = vec![http_json("200 OK", &prm), http_json("200 OK", &as_meta)];
        tokio::spawn(async move {
            for response in responses {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                let mut buf = [0u8; 4096];
                use tokio::io::AsyncReadExt;
                let _ = sock.read(&mut buf).await;
                let _ = sock.write_all(response.as_bytes()).await;
                let _ = sock.shutdown().await;
            }
        });

        let client = OAuthHttpClient::new(Duration::from_secs(2), &origin).unwrap();
        let hint = format!(r#"Bearer resource_metadata="{origin}/prm""#);
        let meta = discover(&client, &format!("{origin}/mcp"), Some(&hint))
            .await
            .unwrap();
        assert_eq!(meta.authorization_endpoint, format!("{origin}/a"));
        // No PRM `resource` field -> canonical MCP URL is used.
        assert_eq!(meta.resource, format!("{origin}/mcp"));
    }

    #[test]
    fn rejects_non_public_ip_ranges() {
        for address in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "224.0.0.1",
            "::1",
            "fe80::1",
            "fd00::1",
            "::ffff:169.254.169.254",
            "64:ff9b::a9fe:a9fe",
            "64:ff9b:1::a9fe:a9fe",
            "2002:a9fe:a9fe::1",
            "2001:0000:a9fe:a9fe::1",
        ] {
            let ip: IpAddr = address.parse().unwrap();
            assert!(!is_public_ip(ip), "unexpectedly allowed {address}");
        }
        assert!(is_public_ip("8.8.8.8".parse().unwrap()));
        assert!(is_public_ip("2606:4700:4700::1111".parse().unwrap()));
    }

    #[test]
    fn canonicalizes_trailing_dot_before_dns_pinning() {
        let mut url = Url::parse("https://as.example.com./metadata").unwrap();
        let origin = Origin::from_url(&url).unwrap();
        canonicalize_request_host(&mut url, &origin).unwrap();
        assert_eq!(url.host_str(), Some("as.example.com"));
        assert_eq!(origin.host, "as.example.com");
    }

    #[test]
    fn validates_endpoint_origin_and_scheme() {
        let issuer = Url::parse("https://as.example.com/tenant").unwrap();
        let valid = json!({
            "issuer": "https://as.example.com/tenant",
            "authorization_endpoint": "https://as.example.com/authorize",
            "token_endpoint": "https://as.example.com/token",
            "registration_endpoint": "https://as.example.com/register"
        });
        assert!(validate_server_metadata(&issuer, &valid).is_ok());
        for (field, endpoint) in [
            ("authorization_endpoint", "http://login.example/authorize"),
            ("token_endpoint", "http://as.example.com/token"),
            ("registration_endpoint", "https://evil.example/register"),
        ] {
            let mut invalid = valid.clone();
            invalid[field] = Value::String(endpoint.to_string());
            assert!(
                validate_server_metadata(&issuer, &invalid).is_err(),
                "unexpectedly accepted {field}={endpoint}"
            );
        }
    }

    #[tokio::test]
    async fn redirect_is_not_followed() {
        let response = "HTTP/1.1 302 Found\r\nlocation: http://169.254.169.254/latest/meta-data/\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".to_string();
        let base = one_shot_server(vec![response]).await;
        let client = OAuthHttpClient::new(Duration::from_secs(2), &base).unwrap();
        let err = fetch_json(&client, &base).await.unwrap_err();
        assert!(err.contains("HTTP 3xx"), "got: {err}");
        assert!(err.contains("Cloudflare Access"), "got: {err}");
    }

    #[tokio::test]
    async fn rejects_http_and_internal_https_destinations() {
        let public_policy =
            OAuthHttpClient::new(Duration::from_secs(2), "https://mcp.example.com/mcp").unwrap();
        assert!(
            public_policy
                .client_for("http://example.com/meta")
                .await
                .is_err()
        );
        assert!(
            public_policy
                .client_for("https://169.254.169.254/meta")
                .await
                .is_err()
        );
        assert!(
            public_policy
                .client_for("https://127.0.0.1/meta")
                .await
                .is_err()
        );
    }
}
