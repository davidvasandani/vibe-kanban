use std::sync::atomic::{AtomicUsize, Ordering};

use axum::{Router, extract::Form, http::HeaderMap, routing::post};
use services::services::mcp_gateway_secrets::McpGatewaySecretStore;
use sqlx::SqlitePool;

use super::{
    super::{persist_credentials_with_store, refresh_credentials_with_store},
    *,
};

struct Upstream {
    url: String,
    refreshes: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Upstream {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Upstream {
    async fn start() -> Self {
        let refreshes = Arc::new(AtomicUsize::new(0));
        let counter = refreshes.clone();
        let app = Router::new()
            .route("/token", post(move |Form(form): Form<std::collections::HashMap<String, String>>| {
                let counter = counter.clone();
                async move {
                    assert_eq!(form.get("grant_type").map(String::as_str), Some("refresh_token"));
                    assert_eq!(form.get("refresh_token").map(String::as_str), Some("retained-grant"));
                    counter.fetch_add(1, Ordering::SeqCst);
                    // Providers may omit a replacement refresh token.
                    Json(json!({"access_token":"renewed", "expires_in":7200, "token_type":"Bearer"}))
                }
            }))
            .route("/mcp", post(|headers: HeaderMap, Json(body): Json<Value>| async move {
                assert_eq!(body["method"], "initialize");
                if headers.get("authorization").and_then(|v| v.to_str().ok()) != Some("Bearer renewed") {
                    return (StatusCode::UNAUTHORIZED, Json(json!({"kind":"token_expired"}))).into_response();
                }
                Json(json!({"jsonrpc":"2.0", "id":1, "result":{"protocolVersion":"2025-03-26", "serverInfo":{}, "capabilities":{}}})).into_response()
            }))
            .route("/cloudflare", post(|| async { (StatusCode::FORBIDDEN, Json(json!({"error":"invalid_token"}))) }))
            .route("/policy", post(|| async { (StatusCode::FORBIDDEN, "edge policy denied") }))
            .route("/unavailable", post(|| async { StatusCode::SERVICE_UNAVAILABLE }))
            .route("/not-mcp", post(|| async { Json(json!({"ok":true})) }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            url,
            refreshes,
            task,
        }
    }
}

fn stored(url: &str) -> StoredCredentials {
    StoredCredentials {
        access_token: "expired".into(),
        refresh_token: Some("retained-grant".into()),
        token_endpoint: Some(format!("{url}/token")),
        revocation_endpoint: None,
        client_id: Some("test-client".into()),
        client_secret: None,
        resource: format!("{url}/mcp"),
        redirect_uri: None,
        scope: Some("openid offline_access".into()),
        cf_access_client_id: None,
        cf_access_client_secret: None,
    }
}
fn row(url: &str) -> McpGatewayConnection {
    McpGatewayConnection {
        id: uuid::Uuid::new_v4().to_string(),
        owner_user_id: "test-user".into(),
        machine_id: "test-machine".into(),
        server_name: "sgsc-mcp".into(),
        upstream_url: format!("{url}/mcp"),
        transport: "http".into(),
        auth_kind: "oauth".into(),
        gateway_token_hash: vec![42; 32],
        encrypted_credentials: None,
        credential_version: 0,
        status: "connected".into(),
        expires_at: Some("2020-01-01T00:00:00Z".into()),
        last_error_code: None,
    }
}
async fn fixture(
    url: &str,
    store: &McpGatewaySecretStore,
) -> (SqlitePool, McpGatewayConnection, StoredCredentials) {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../../db/migrations/20260715000000_mcp_gateway.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let mut row = row(url);
    let credentials = stored(url);
    row.encrypted_credentials = Some(
        store
            .encrypt(
                &serde_json::to_vec(&credentials).unwrap(),
                binding(&row).as_bytes(),
            )
            .unwrap(),
    );
    sqlx::query("INSERT INTO mcp_gateway_connections (id,owner_user_id,machine_id,server_name,upstream_url,transport,auth_kind,gateway_token_hash,encrypted_credentials,status,expires_at) VALUES (?,?,?,?,?,'http','oauth',?,?,'connected',?)")
        .bind(&row.id).bind(&row.owner_user_id).bind(&row.machine_id).bind(&row.server_name).bind(&row.upstream_url)
        .bind(&row.gateway_token_hash).bind(&row.encrypted_credentials).bind(&row.expires_at).execute(&pool).await.unwrap();
    (pool, row, credentials)
}
async fn reload(pool: &SqlitePool, row: &McpGatewayConnection) -> McpGatewayConnection {
    McpGatewayConnection::find_bound(pool, &row.id, &row.owner_user_id, &row.machine_id)
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn expired_grant_renews_without_rotating_agent_capability() {
    let upstream = Upstream::start().await;
    let dir = tempfile::tempdir().unwrap();
    let store = McpGatewaySecretStore::load_or_generate(&dir.path().join("key")).unwrap();
    let (pool, row, stale) = fixture(&upstream.url, &store).await;
    assert_eq!(probe(&row, &stale).await, Probe::Rejected);
    let (first, concurrent) = tokio::join!(
        refresh_credentials_with_store(&pool, &row, &stale, &store),
        refresh_credentials_with_store(&pool, &row, &stale, &store),
    );
    let renewed = first.unwrap();
    assert_eq!(concurrent.unwrap().access_token, renewed.access_token);
    let current = reload(&pool, &row).await;
    assert_eq!(probe(&current, &renewed).await, Probe::Authenticated);
    assert_eq!(current.gateway_token_hash, row.gateway_token_hash);
    assert_eq!(current.credential_version, 1);
    assert_eq!(renewed.refresh_token.as_deref(), Some("retained-grant"));
    let expiry = expiry(&current, &renewed).unwrap();
    assert!(expiry > Utc::now() + chrono::Duration::minutes(115));
    assert!(expiry <= Utc::now() + chrono::Duration::minutes(120));
    let decrypted: StoredCredentials = serde_json::from_slice(
        &store
            .decrypt(
                current.encrypted_credentials.as_deref().unwrap(),
                binding(&current).as_bytes(),
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(decrypted.access_token, "renewed");
    assert_eq!(decrypted.refresh_token, renewed.refresh_token);
    // A concurrent request holding the expired snapshot reuses the saved grant.
    refresh_credentials_with_store(&pool, &row, &stale, &store)
        .await
        .unwrap();
    assert_eq!(upstream.refreshes.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn credential_cas_rejects_stale_updates_and_disconnects() {
    let dir = tempfile::tempdir().unwrap();
    let store = McpGatewaySecretStore::load_or_generate(&dir.path().join("key")).unwrap();
    let (pool, row, credentials) = fixture("http://127.0.0.1:1", &store).await;
    persist_credentials_with_store(&pool, &row, &credentials, None, &store)
        .await
        .unwrap();
    assert!(
        persist_credentials_with_store(&pool, &row, &credentials, None, &store)
            .await
            .is_err()
    );
    let current = reload(&pool, &row).await;
    McpGatewayConnection::disconnect(&pool, &row.id, &row.owner_user_id, &row.machine_id)
        .await
        .unwrap();
    assert!(
        persist_credentials_with_store(&pool, &current, &credentials, None, &store)
            .await
            .is_err()
    );
    assert!(
        refresh_credentials_with_store(&pool, &current, &credentials, &store)
            .await
            .is_err()
    );
    let disconnected = reload(&pool, &row).await;
    assert_eq!(disconnected.status, "disconnected");
    assert!(disconnected.encrypted_credentials.is_none());
}

#[tokio::test]
async fn probe_distinguishes_rejection_from_transport_policy_and_protocol_failure() {
    let upstream = Upstream::start().await;
    let mut row = row(&upstream.url);
    let credentials = stored(&upstream.url);
    for (path, expected) in [
        ("cloudflare", Probe::Rejected),
        ("policy", Probe::Unknown),
        ("unavailable", Probe::Unknown),
        ("not-mcp", Probe::Unknown),
    ] {
        row.upstream_url = format!("{}/{path}", upstream.url);
        assert_eq!(probe(&row, &credentials).await, expected, "{path}");
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    row.upstream_url = format!("http://{}/mcp", listener.local_addr().unwrap());
    drop(listener);
    assert_eq!(probe(&row, &credentials).await, Probe::Unknown);
}

#[test]
fn expiry_and_protocol_evidence() {
    let token = format!("x.{}.x", URL_SAFE_NO_PAD.encode(br#"{"exp":2000000000}"#));
    assert_eq!(
        token_expiry(None, &token),
        Some("2033-05-18T03:33:20+00:00".into())
    );
    assert!(due(
        Some(Utc::now() + chrono::Duration::minutes(5)),
        Utc::now()
    ));
    assert!(!due(
        Some(Utc::now() + chrono::Duration::days(1)),
        Utc::now()
    ));
    assert!(!due(None, Utc::now()));
    assert!(!initialized_body(
        br#"{"jsonrpc":"2.0","id":1,"error":{"code":-1}}"#
    ));
    assert!(!initialized_body(br#"{"ok":true}"#));
    assert!(initialized_body(b"event: message\ndata: {\"id\":1,\"result\":{\"protocolVersion\":\"2025-03-26\",\"serverInfo\":{},\"capabilities\":{}}}\n\n"));
    assert_eq!(
        auth_kind(br#"{"kind":"token_expired"}"#),
        Some("token_expired")
    );
    assert_eq!(
        auth_kind(br#"{"error":"invalid_token"}"#),
        Some("invalid_token")
    );
    assert!(!refresh_rejected("token refresh failed (HTTP 503)"));
}

#[tokio::test]
async fn inventory_is_bound_and_disconnected_entries_are_absent() {
    let dir = tempfile::tempdir().unwrap();
    let store = McpGatewaySecretStore::load_or_generate(&dir.path().join("key")).unwrap();
    let (pool, row, _) = fixture("http://127.0.0.1:1", &store).await;
    let rows = bound_rows(&pool, &row.owner_user_id, &row.machine_id)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(target_name(&row, &rows), "sgsc-mcp");
    assert!(
        bound_rows(&pool, "other", &row.machine_id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        bound_rows(&pool, &row.owner_user_id, "other")
            .await
            .unwrap()
            .is_empty()
    );
    let mut duplicate = row.clone();
    duplicate.id = uuid::Uuid::new_v4().to_string();
    assert_eq!(target_name(&row, &[row.clone(), duplicate]), row.id);
    McpGatewayConnection::disconnect(&pool, &row.id, &row.owner_user_id, &row.machine_id)
        .await
        .unwrap();
    assert!(
        bound_rows(&pool, &row.owner_user_id, &row.machine_id)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn authentication_errors_name_the_gateway_target_and_preserve_diagnostics() {
    for kind in ["token_expired", "invalid_token"] {
        let response = (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"Invalid or expired token", "kind":kind})),
        )
            .into_response();
        let response = auth_error_response(response, "sgsc-mcp").await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["kind"], kind);
        assert_eq!(value["error"], "Invalid or expired token");
        assert_eq!(value["reauth_target"], "mcp-gateway:sgsc-mcp");
    }
    let response = auth_error_response(
        (StatusCode::FORBIDDEN, "edge policy denied").into_response(),
        "sgsc-mcp",
    )
    .await;
    assert_eq!(
        to_bytes(response.into_body(), 4096).await.unwrap(),
        "edge policy denied"
    );
}

#[tokio::test]
async fn reconnect_to_another_upstream_never_returns_its_token_to_a_stale_proxy() {
    let dir = tempfile::tempdir().unwrap();
    let store = McpGatewaySecretStore::load_or_generate(&dir.path().join("key")).unwrap();
    let (pool, row, stale) = fixture("http://127.0.0.1:1", &store).await;
    let mut moved = row.clone();
    moved.upstream_url = "http://127.0.0.1:2/mcp".into();
    let mut new_credentials = stale.clone();
    new_credentials.access_token = "different-upstream-token".into();
    let envelope = store
        .encrypt(
            &serde_json::to_vec(&new_credentials).unwrap(),
            binding(&moved).as_bytes(),
        )
        .unwrap();
    sqlx::query("UPDATE mcp_gateway_connections SET upstream_url = ?, encrypted_credentials = ?, credential_version = credential_version + 1 WHERE id = ?")
        .bind(&moved.upstream_url).bind(envelope).bind(&row.id).execute(&pool).await.unwrap();
    assert!(
        refresh_credentials_with_store(&pool, &row, &stale, &store)
            .await
            .is_err()
    );
}
