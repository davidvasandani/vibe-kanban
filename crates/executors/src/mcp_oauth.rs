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

use base64::Engine;
use rand::{Rng, distributions::Alphanumeric};
use reqwest::{Url, header::WWW_AUTHENTICATE};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// Everything discovery learns that the authorize/exchange steps need.
#[derive(Debug, Clone)]
pub struct AuthServerMetadata {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub registration_endpoint: Option<String>,
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

/// GET a JSON document, treating any non-success status or parse failure as
/// a soft error (the caller tries the next candidate URL).
async fn fetch_json(client: &reqwest::Client, url: &str) -> Result<Value, String> {
    let resp = client
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| format!("request to {url} failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("{url} returned HTTP {}", resp.status().as_u16()));
    }
    resp.json::<Value>()
        .await
        .map_err(|e| format!("{url} returned invalid JSON: {e}"))
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
    client: &reqwest::Client,
    mcp_url: &str,
    www_authenticate_hint: Option<&str>,
) -> Result<AuthServerMetadata, String> {
    let base = Url::parse(mcp_url).map_err(|e| format!("invalid server url `{mcp_url}`: {e}"))?;

    // 1. Find the protected-resource metadata URL: header hint, fresh 401,
    //    then the spec's well-known fallbacks.
    let mut prm_candidates: Vec<String> = Vec::new();
    if let Some(url) = www_authenticate_hint.and_then(parse_resource_metadata_url) {
        prm_candidates.push(url);
    } else if let Ok(resp) = client.get(mcp_url).send().await
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
    let mut as_candidates = well_known_candidates(&as_url, "oauth-authorization-server");
    as_candidates.extend(well_known_candidates(&as_url, "openid-configuration"));

    let mut as_errors = Vec::new();
    for candidate in &as_candidates {
        match fetch_json(client, candidate).await {
            Ok(meta) => {
                let authorization_endpoint = string_field(&meta, "authorization_endpoint");
                let token_endpoint = string_field(&meta, "token_endpoint");
                if let (Some(authorization_endpoint), Some(token_endpoint)) =
                    (authorization_endpoint, token_endpoint)
                {
                    return Ok(AuthServerMetadata {
                        authorization_endpoint,
                        token_endpoint,
                        registration_endpoint: string_field(&meta, "registration_endpoint"),
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
    client: &reqwest::Client,
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
    let resp = client
        .post(registration_endpoint)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("client registration request failed: {e}"))?;
    let status = resp.status();
    let value: Value = resp
        .json()
        .await
        .map_err(|e| format!("client registration returned invalid JSON: {e}"))?;
    if !status.is_success() {
        return Err(format!(
            "client registration failed (HTTP {}): {}",
            status.as_u16(),
            string_field(&value, "error_description")
                .or_else(|| string_field(&value, "error"))
                .unwrap_or_else(|| "unknown error".to_string())
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

/// Exchange an authorization code for an access token. Returns only the
/// access token string so the secret spreads no further than necessary.
pub async fn exchange_code(
    client: &reqwest::Client,
    token_endpoint: &str,
    client_id: &str,
    code: &str,
    pkce_verifier: &str,
    redirect_uri: &str,
    resource: &str,
) -> Result<String, String> {
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
    let resp = client
        .post(token_endpoint)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await
        .map_err(|e| format!("token request failed: {e}"))?;
    let status = resp.status();
    let value: Value = resp
        .json()
        .await
        .map_err(|e| format!("token endpoint returned invalid JSON: {e}"))?;
    if !status.is_success() {
        // Error bodies (RFC 6749 §5.2) contain no secrets; token bodies do,
        // so only the error branch echoes response content.
        return Err(format!(
            "token exchange failed (HTTP {}): {}",
            status.as_u16(),
            string_field(&value, "error_description")
                .or_else(|| string_field(&value, "error"))
                .unwrap_or_else(|| "unknown error".to_string())
        ));
    }
    string_field(&value, "access_token")
        .ok_or_else(|| "token response lacks access_token".to_string())
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
        let client = reqwest::Client::new();
        let client_id = register_client(&client, &base, "http://127.0.0.1:1/cb")
            .await
            .unwrap();
        assert_eq!(client_id, "abc123");
    }

    #[tokio::test]
    async fn register_client_surfaces_error_description() {
        let base = one_shot_server(vec![http_json(
            "400 Bad Request",
            r#"{"error":"invalid_redirect_uri","error_description":"loopback not allowed"}"#,
        )])
        .await;
        let client = reqwest::Client::new();
        let err = register_client(&client, &base, "http://127.0.0.1:1/cb")
            .await
            .unwrap_err();
        assert!(err.contains("loopback not allowed"), "got: {err}");
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
        let client = reqwest::Client::new();
        let token = exchange_code(&client, &base, "c", "code", "v", "http://cb", "res")
            .await
            .unwrap();
        assert_eq!(token, "tok-1");
        let err = exchange_code(&client, &base, "c", "code", "v", "http://cb", "res")
            .await
            .unwrap_err();
        assert!(err.contains("invalid_grant"), "got: {err}");
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

        let client = reqwest::Client::new();
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

        let client = reqwest::Client::new();
        let hint = format!(r#"Bearer resource_metadata="{origin}/prm""#);
        let meta = discover(&client, "https://unreachable.invalid/mcp", Some(&hint))
            .await
            .unwrap();
        assert_eq!(meta.authorization_endpoint, format!("{origin}/a"));
        // No PRM `resource` field -> canonical MCP URL is used.
        assert_eq!(meta.resource, "https://unreachable.invalid/mcp");
    }
}
