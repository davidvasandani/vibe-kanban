use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
    response::Response,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

use super::*;

const ISSUER: &str = "https://vibe.example.test";
const REDIRECT: &str = "https://chatgpt.com/connector_platform_oauth_redirect";
const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

async fn app(enabled: bool) -> Router {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
    let config = enabled.then(|| McpOAuthConfig::parse(ISSUER).unwrap());
    let state = McpOAuthState::new(pool, config);
    public_router()
        .with_state(state.clone())
        .nest_service("/api/mcp-oauth", api_router().with_state(state))
}

async fn send(app: &Router, request: Request<Body>) -> Response {
    app.clone().oneshot(request).await.unwrap()
}

async fn body_text(response: Response) -> String {
    String::from_utf8(
        to_bytes(response.into_body(), 1 << 20)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

async fn body_json(response: Response) -> Value {
    serde_json::from_str(&body_text(response).await).unwrap()
}

fn get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

fn post_json(uri: &str, body: Value) -> Request<Body> {
    Request::post(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn post_form(uri: &str, params: &[(&str, &str)]) -> Request<Body> {
    post_form_with(uri, params, None)
}

fn post_form_with(uri: &str, params: &[(&str, &str)], auth: Option<String>) -> Request<Body> {
    let body = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(params)
        .finish();
    let mut builder =
        Request::post(uri).header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
    if let Some(auth) = auth {
        builder = builder.header(header::AUTHORIZATION, auth);
    }
    builder.body(Body::from(body)).unwrap()
}

fn bearer(uri: &str, token: &str) -> Request<Body> {
    Request::get(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

fn location(response: &Response) -> Url {
    Url::parse(
        response
            .headers()
            .get(header::LOCATION)
            .expect("redirect")
            .to_str()
            .unwrap(),
    )
    .unwrap()
}

fn query_param(url: &Url, key: &str) -> Option<String> {
    url.query_pairs()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.into_owned())
}

fn hidden_field(html: &str, name: &str) -> String {
    let marker = format!("name=\"{name}\" value=\"");
    let start = html.find(&marker).expect("hidden field") + marker.len();
    let end = start + html[start..].find('"').unwrap();
    html[start..end].to_string()
}

async fn register(app: &Router, body: Value) -> Value {
    let response = send(app, post_json("/oauth/register", body)).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    body_json(response).await
}

async fn register_public(app: &Router) -> String {
    let client = register(
        app,
        json!({
            "client_name": "ChatGPT",
            "redirect_uris": [REDIRECT],
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
            "token_endpoint_auth_method": "none",
        }),
    )
    .await;
    assert!(client.get("client_secret").is_none());
    client["client_id"].as_str().unwrap().to_string()
}

fn authorize_uri(client_id: &str, extra: &[(&str, &str)]) -> String {
    let mut params = vec![
        ("response_type", "code"),
        ("client_id", client_id),
        ("redirect_uri", REDIRECT),
        ("code_challenge", CHALLENGE),
        ("code_challenge_method", "S256"),
        ("state", "xyz"),
        ("scope", "mcp"),
        ("resource", "https://vibe.example.test/oauth/mcp"),
    ];
    for (key, value) in extra {
        params.retain(|(k, _)| k != key);
        if !value.is_empty() {
            params.push((key, value));
        }
    }
    let query = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(params)
        .finish();
    format!("/oauth/authorize?{query}")
}

/// Returns (request_id, consent) from the rendered consent form.
async fn open_consent(app: &Router, client_id: &str) -> (String, String) {
    let response = send(app, get(&authorize_uri(client_id, &[]))).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::X_FRAME_OPTIONS], "DENY");
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let html = body_text(response).await;
    assert!(html.contains("ChatGPT"));
    (
        hidden_field(&html, "request_id"),
        hidden_field(&html, "consent"),
    )
}

async fn approve(app: &Router, client_id: &str) -> String {
    let (request_id, consent) = open_consent(app, client_id).await;
    let response = send(
        app,
        post_form(
            "/oauth/authorize",
            &[
                ("request_id", &request_id),
                ("consent", &consent),
                ("decision", "approve"),
            ],
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FOUND);
    let target = location(&response);
    assert!(target.as_str().starts_with(REDIRECT));
    assert_eq!(query_param(&target, "state").as_deref(), Some("xyz"));
    assert_eq!(query_param(&target, "iss").as_deref(), Some(ISSUER));
    query_param(&target, "code").unwrap()
}

async fn exchange(app: &Router, client_id: &str, code: &str) -> Response {
    send(
        app,
        post_form(
            "/oauth/token",
            &[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", REDIRECT),
                ("code_verifier", VERIFIER),
                ("client_id", client_id),
            ],
        ),
    )
    .await
}

async fn tokens(app: &Router, client_id: &str) -> (String, String) {
    let code = approve(app, client_id).await;
    let response = exchange(app, client_id, &code).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let body = body_json(response).await;
    assert_eq!(body["token_type"], "Bearer");
    assert_eq!(body["expires_in"], 3600);
    assert_eq!(body["scope"], "mcp");
    (
        body["access_token"].as_str().unwrap().to_string(),
        body["refresh_token"].as_str().unwrap().to_string(),
    )
}

async fn verify_status(app: &Router, token: &str) -> StatusCode {
    send(app, bearer("/api/mcp-oauth/verify", token))
        .await
        .status()
}

#[tokio::test]
async fn disabled_server_exposes_nothing() {
    let app = app(false).await;
    for uri in [
        "/.well-known/oauth-protected-resource/oauth/mcp",
        "/.well-known/oauth-authorization-server",
        "/oauth/authorize?client_id=x",
        "/api/mcp-oauth/verify",
        "/api/mcp-oauth/grants",
    ] {
        assert_eq!(
            send(&app, get(uri)).await.status(),
            StatusCode::NOT_FOUND,
            "{uri}"
        );
    }
    let response = send(
        &app,
        post_json("/oauth/register", json!({"redirect_uris": [REDIRECT]})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = send(
        &app,
        post_form("/oauth/token", &[("grant_type", "refresh_token")]),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn metadata_describes_the_server() {
    let app = app(true).await;
    for uri in [
        "/.well-known/oauth-protected-resource",
        "/.well-known/oauth-protected-resource/oauth/mcp",
    ] {
        let body = body_json(send(&app, get(uri)).await).await;
        assert_eq!(body["resource"], "https://vibe.example.test/oauth/mcp");
        assert_eq!(body["authorization_servers"], json!([ISSUER]));
        assert_eq!(body["scopes_supported"], json!(["mcp"]));
    }
    for uri in [
        "/.well-known/oauth-authorization-server",
        "/.well-known/oauth-authorization-server/oauth/mcp",
    ] {
        let body = body_json(send(&app, get(uri)).await).await;
        assert_eq!(body["issuer"], ISSUER);
        assert_eq!(
            body["registration_endpoint"],
            "https://vibe.example.test/oauth/register"
        );
        assert_eq!(
            body["authorization_endpoint"],
            "https://vibe.example.test/oauth/authorize"
        );
        assert_eq!(
            body["token_endpoint"],
            "https://vibe.example.test/oauth/token"
        );
        assert_eq!(body["code_challenge_methods_supported"], json!(["S256"]));
        assert_eq!(body["response_types_supported"], json!(["code"]));
    }
}

#[tokio::test]
async fn registration_validates_metadata() {
    let app = app(true).await;
    let cases = [
        (json!({}), "invalid_redirect_uri"),
        (json!({"redirect_uris": []}), "invalid_redirect_uri"),
        (
            json!({"redirect_uris": ["http://evil.example/cb"]}),
            "invalid_redirect_uri",
        ),
        (
            json!({"redirect_uris": ["https://a.example/cb#frag"]}),
            "invalid_redirect_uri",
        ),
        (
            json!({"redirect_uris": ["relative/cb"]}),
            "invalid_redirect_uri",
        ),
        (
            json!({"redirect_uris": [REDIRECT], "grant_types": ["client_credentials"]}),
            "invalid_client_metadata",
        ),
        (
            json!({"redirect_uris": [REDIRECT], "response_types": ["token"]}),
            "invalid_client_metadata",
        ),
        (
            json!({"redirect_uris": [REDIRECT], "token_endpoint_auth_method": "private_key_jwt"}),
            "invalid_client_metadata",
        ),
        (
            json!({"redirect_uris": [REDIRECT], "client_name": "x".repeat(201)}),
            "invalid_client_metadata",
        ),
        (json!([1, 2]), "invalid_client_metadata"),
    ];
    for (body, error) in cases {
        let response = send(&app, post_json("/oauth/register", body.clone())).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body_json(response).await["error"], error, "{body}");
    }
    let oversized = json!({"redirect_uris": [REDIRECT], "client_name": "x".repeat(20_000)});
    let response = send(&app, post_json("/oauth/register", oversized)).await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);

    // Loopback http is allowed for native clients.
    register(&app, json!({"redirect_uris": ["http://127.0.0.1:7777/cb"]})).await;
    // Omitting the method defaults to client_secret_basic, which issues a secret.
    let confidential = register(&app, json!({"redirect_uris": [REDIRECT]})).await;
    assert_eq!(
        confidential["token_endpoint_auth_method"],
        "client_secret_basic"
    );
    assert!(confidential["client_secret"].as_str().is_some());
    assert_eq!(confidential["client_secret_expires_at"], 0);
}

#[tokio::test]
async fn authorize_never_redirects_to_unproven_targets() {
    let app = app(true).await;
    let client_id = register_public(&app).await;

    let response = send(&app, get(&authorize_uri("unknown", &[]))).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(response.headers().get(header::LOCATION).is_none());

    let response = send(
        &app,
        get(&authorize_uri(
            &client_id,
            &[("redirect_uri", "https://evil.example/cb")],
        )),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(response.headers().get(header::LOCATION).is_none());
}

#[tokio::test]
async fn authorize_reports_protocol_errors_to_the_client() {
    let app = app(true).await;
    let client_id = register_public(&app).await;
    let cases: [(&[(&str, &str)], &str); 6] = [
        (&[("response_type", "token")], "unsupported_response_type"),
        (&[("code_challenge", "")], "invalid_request"),
        (&[("code_challenge", "short")], "invalid_request"),
        (&[("code_challenge_method", "plain")], "invalid_request"),
        (&[("scope", "mcp admin")], "invalid_scope"),
        (
            &[("resource", "https://other.example/mcp")],
            "invalid_target",
        ),
    ];
    for (extra, error) in cases {
        let response = send(&app, get(&authorize_uri(&client_id, extra))).await;
        assert_eq!(response.status(), StatusCode::FOUND, "{extra:?}");
        let target = location(&response);
        assert!(target.as_str().starts_with(REDIRECT));
        assert_eq!(
            query_param(&target, "error").as_deref(),
            Some(error),
            "{extra:?}"
        );
        assert_eq!(query_param(&target, "state").as_deref(), Some("xyz"));
        assert_eq!(query_param(&target, "iss").as_deref(), Some(ISSUER));
    }
    // A single registered redirect URI may be omitted.
    let response = send(
        &app,
        get(&authorize_uri(&client_id, &[("redirect_uri", "")])),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn consent_requires_the_one_time_token() {
    let app = app(true).await;
    let client_id = register_public(&app).await;
    let (request_id, consent) = open_consent(&app, &client_id).await;

    let forged = send(
        &app,
        post_form(
            "/oauth/authorize",
            &[
                ("request_id", &request_id),
                ("consent", "forged"),
                ("decision", "approve"),
            ],
        ),
    )
    .await;
    assert_eq!(forged.status(), StatusCode::BAD_REQUEST);
    assert!(forged.headers().get(header::LOCATION).is_none());

    let denied = send(
        &app,
        post_form(
            "/oauth/authorize",
            &[
                ("request_id", &request_id),
                ("consent", &consent),
                ("decision", "deny"),
            ],
        ),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::FOUND);
    let target = location(&denied);
    assert_eq!(
        query_param(&target, "error").as_deref(),
        Some("access_denied")
    );
    assert_eq!(query_param(&target, "state").as_deref(), Some("xyz"));

    // The same consent cannot be reused after it was resolved.
    let reused = send(
        &app,
        post_form(
            "/oauth/authorize",
            &[
                ("request_id", &request_id),
                ("consent", &consent),
                ("decision", "approve"),
            ],
        ),
    )
    .await;
    assert_eq!(reused.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn code_exchange_checks_pkce_redirect_and_client_and_revokes_on_replay() {
    let app = app(true).await;
    let client_id = register_public(&app).await;
    let other_client = register_public(&app).await;
    let code = approve(&app, &client_id).await;

    let mismatches: [&[(&str, &str)]; 3] = [
        &[("code_verifier", &"a".repeat(43))],
        &[("redirect_uri", "https://chatgpt.com/other")],
        &[("client_id", &other_client)],
    ];
    for overrides in mismatches {
        let mut params = vec![
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("redirect_uri", REDIRECT),
            ("code_verifier", VERIFIER),
            ("client_id", client_id.as_str()),
        ];
        for (key, value) in overrides {
            params.retain(|(k, _)| k != key);
            params.push((key, value));
        }
        let response = send(&app, post_form("/oauth/token", &params)).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{overrides:?}");
        assert_eq!(body_json(response).await["error"], "invalid_grant");
    }

    let response = exchange(&app, &client_id, &code).await;
    assert_eq!(response.status(), StatusCode::OK);
    let access = body_json(response).await["access_token"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(verify_status(&app, &access).await, StatusCode::NO_CONTENT);

    // Replaying the code fails and revokes what it minted.
    let replay = exchange(&app, &client_id, &code).await;
    assert_eq!(replay.status(), StatusCode::BAD_REQUEST);
    assert_eq!(body_json(replay).await["error"], "invalid_grant");
    assert_eq!(verify_status(&app, &access).await, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn token_endpoint_rejects_malformed_requests() {
    let app = app(true).await;
    let client_id = register_public(&app).await;
    let response = send(
        &app,
        post_form(
            "/oauth/token",
            &[("grant_type", "password"), ("client_id", &client_id)],
        ),
    )
    .await;
    assert_eq!(body_json(response).await["error"], "unsupported_grant_type");

    let response = send(
        &app,
        post_form(
            "/oauth/token",
            &[
                ("grant_type", "authorization_code"),
                ("client_id", &client_id),
            ],
        ),
    )
    .await;
    assert_eq!(body_json(response).await["error"], "invalid_request");

    let response = send(
        &app,
        post_form("/oauth/token", &[("grant_type", "authorization_code")]),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(body_json(response).await["error"], "invalid_client");

    let response = send(
        &app,
        post_form(
            "/oauth/token",
            &[
                ("grant_type", "refresh_token"),
                ("client_id", &client_id),
                ("refresh_token", "x"),
                ("resource", "https://other.example/mcp"),
            ],
        ),
    )
    .await;
    assert_eq!(body_json(response).await["error"], "invalid_target");
}

#[tokio::test]
async fn confidential_clients_must_authenticate() {
    let app = app(true).await;
    let client = register(
        &app,
        json!({"client_name": "ChatGPT", "redirect_uris": [REDIRECT]}),
    )
    .await;
    let client_id = client["client_id"].as_str().unwrap().to_string();
    let secret = client["client_secret"].as_str().unwrap().to_string();

    let code = approve(&app, &client_id).await;
    let params = [
        ("grant_type", "authorization_code"),
        ("code", code.as_str()),
        ("redirect_uri", REDIRECT),
        ("code_verifier", VERIFIER),
    ];

    // No secret at all (a client that ignored its issued secret).
    let mut with_id = params.to_vec();
    with_id.push(("client_id", &client_id));
    let response = send(&app, post_form("/oauth/token", &with_id)).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(body_json(response).await["error"], "invalid_client");

    // Wrong secret over Basic gets a Basic challenge.
    let wrong = format!("Basic {}", STANDARD.encode(format!("{client_id}:nope")));
    let response = send(&app, post_form_with("/oauth/token", &params, Some(wrong))).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(response.headers().contains_key(header::WWW_AUTHENTICATE));

    // Correct Basic credentials succeed.
    let basic = format!("Basic {}", STANDARD.encode(format!("{client_id}:{secret}")));
    let response = send(&app, post_form_with("/oauth/token", &params, Some(basic))).await;
    assert_eq!(response.status(), StatusCode::OK);
    let refresh = body_json(response).await["refresh_token"]
        .as_str()
        .unwrap()
        .to_string();

    // client_secret_post is accepted for the same client.
    let response = send(
        &app,
        post_form(
            "/oauth/token",
            &[
                ("grant_type", "refresh_token"),
                ("refresh_token", &refresh),
                ("client_id", &client_id),
                ("client_secret", &secret),
            ],
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn refresh_rotates_and_reuse_revokes_the_grant() {
    let app = app(true).await;
    let client_id = register_public(&app).await;
    let (_, refresh) = tokens(&app, &client_id).await;

    let refresh_with = |token: String| {
        post_form(
            "/oauth/token",
            &[
                ("grant_type", "refresh_token"),
                ("refresh_token", &token),
                ("client_id", &client_id),
            ],
        )
    };
    let response = send(&app, refresh_with(refresh.clone())).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    let new_access = body["access_token"].as_str().unwrap().to_string();
    let new_refresh = body["refresh_token"].as_str().unwrap().to_string();
    assert_ne!(new_refresh, refresh);
    assert_eq!(
        verify_status(&app, &new_access).await,
        StatusCode::NO_CONTENT
    );

    // Reusing the rotated refresh token revokes the whole grant.
    let reuse = send(&app, refresh_with(refresh)).await;
    assert_eq!(body_json(reuse).await["error"], "invalid_grant");
    assert_eq!(
        verify_status(&app, &new_access).await,
        StatusCode::UNAUTHORIZED
    );
    let after = send(&app, refresh_with(new_refresh)).await;
    assert_eq!(body_json(after).await["error"], "invalid_grant");
}

#[tokio::test]
async fn verify_challenges_point_at_resource_metadata() {
    let app = app(true).await;
    let client_id = register_public(&app).await;
    let (access, refresh) = tokens(&app, &client_id).await;

    let missing = send(&app, get("/api/mcp-oauth/verify")).await;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);
    let challenge = missing.headers()[header::WWW_AUTHENTICATE]
        .to_str()
        .unwrap()
        .to_string();
    assert_eq!(
        challenge,
        "Bearer resource_metadata=\"https://vibe.example.test/.well-known/oauth-protected-resource/oauth/mcp\""
    );
    let body = body_json(missing).await;
    assert_eq!(body["jsonrpc"], "2.0");
    assert_eq!(body["error"]["code"], -32001);

    for token in ["garbage", refresh.as_str()] {
        let response = send(&app, bearer("/api/mcp-oauth/verify", token)).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(
            response.headers()[header::WWW_AUTHENTICATE]
                .to_str()
                .unwrap()
                .contains("error=\"invalid_token\"")
        );
    }

    // forward_auth may replay any method; verify answers all of them.
    let post = Request::post("/api/mcp-oauth/verify")
        .header(header::AUTHORIZATION, format!("Bearer {access}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&app, post).await.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn grants_are_listed_and_revocable() {
    let app = app(true).await;
    let client_id = register_public(&app).await;
    let (access, _) = tokens(&app, &client_id).await;

    let listed = body_json(send(&app, get("/api/mcp-oauth/grants")).await).await;
    let grants = listed["data"].as_array().unwrap();
    assert_eq!(grants.len(), 1);
    assert_eq!(grants[0]["client_name"], "ChatGPT");
    let grant_id = grants[0]["id"].as_str().unwrap().to_string();

    let delete = |id: &str| {
        Request::delete(format!("/api/mcp-oauth/grants/{id}"))
            .body(Body::empty())
            .unwrap()
    };
    assert_eq!(send(&app, delete(&grant_id)).await.status(), StatusCode::OK);
    assert_eq!(verify_status(&app, &access).await, StatusCode::UNAUTHORIZED);
    assert_eq!(
        send(&app, delete(&grant_id)).await.status(),
        StatusCode::NOT_FOUND
    );
    let listed = body_json(send(&app, get("/api/mcp-oauth/grants")).await).await;
    assert!(listed["data"].as_array().unwrap().is_empty());
}
