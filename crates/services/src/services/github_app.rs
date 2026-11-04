//! GitHub App keys stay on the coordinator. Only installation tokens leave it.
use std::{
    collections::HashMap,
    sync::{
        Arc, LazyLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use chrono::{DateTime, Utc};
use db::models::github_owner_token::GitHubOwnerTokenRow;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use ts_rs::TS;
use utils::github_credentials::GitHubToken;

use super::github_owner_tokens::GitHubOwnerTokenError;

const PREFIX: &str = "vk-github-app-v1:";
const MARGIN: i64 = 300;
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
pub struct AppConfig {
    pub app_id: String,
    pub installation_id: String,
    pub private_key_reference: String,
}

pub fn parse_config(value: &str) -> Option<AppConfig> {
    let app = serde_json::from_str(value.strip_prefix(PREFIX)?).ok()?;
    let canonical = encode_config(app, "").ok()?;
    serde_json::from_str(canonical.strip_prefix(PREFIX)?).ok()
}

pub fn encode_config(mut app: AppConfig, pat: &str) -> Result<String, GitHubOwnerTokenError> {
    let id_valid = |s: &str| {
        !s.is_empty()
            && s.bytes().all(|b| b.is_ascii_digit())
            && s.parse::<u64>().is_ok_and(|n| n > 0)
    };
    if !pat.trim().is_empty() || !id_valid(&app.app_id) || !id_valid(&app.installation_id) {
        return Err(GitHubOwnerTokenError::InvalidApp);
    }
    app.app_id = app.app_id.parse::<u64>().unwrap().to_string();
    app.installation_id = app.installation_id.parse::<u64>().unwrap().to_string();
    app.private_key_reference = api_types::normalize_secret_reference(&app.private_key_reference)
        .filter(|s| {
            let parts: Vec<_> = s.trim_start_matches("op://").split('/').collect();
            s.len() <= 4096
                && !s.chars().any(char::is_control)
                && parts.len() >= 3
                && parts.iter().all(|part| !part.trim().is_empty())
        })
        .ok_or(GitHubOwnerTokenError::InvalidApp)?;
    Ok(format!(
        "{PREFIX}{}",
        serde_json::to_string(&app).map_err(|_| GitHubOwnerTokenError::InvalidApp)?
    ))
}

#[derive(Debug, Clone, Default, Serialize, TS)]
pub struct AppHealth {
    pub app_reachable: bool,
    pub installation_found: bool,
    pub token_minted: bool,
    pub missing_permissions: Vec<String>,
}
#[derive(Clone)]
pub struct InstallationToken {
    pub token: GitHubToken,
    pub expires_at: DateTime<Utc>,
    pub health: AppHealth,
}
#[derive(Debug, Clone)]
pub struct AppFailure {
    pub detail: String,
    pub health: AppHealth,
}
impl AppFailure {
    fn at(health: &AppHealth, detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
            health: health.clone(),
        }
    }
}
// Serialize refreshes per credential, never across owners. Settings invalidation never
// waits for network I/O and prevents an in-flight result from being returned.
type TokenSlot = Arc<Mutex<Option<(String, InstallationToken)>>>;
#[derive(Default)]
struct TokenCache {
    entries: Mutex<HashMap<uuid::Uuid, TokenSlot>>,
    generation: AtomicU64,
}
static CACHE: LazyLock<TokenCache> = LazyLock::new(TokenCache::default);
pub fn invalidate() {
    CACHE.generation.fetch_add(1, Ordering::SeqCst);
}
impl TokenCache {
    async fn get_or_mint<F, Fut>(
        &self,
        row: &GitHubOwnerTokenRow,
        force_refresh: bool,
        mint: F,
    ) -> Result<InstallationToken, AppFailure>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<InstallationToken, AppFailure>>,
    {
        let slot = {
            let mut entries = self.entries.lock().await;
            // Retire idle expired/failed entries without awaiting their locks.
            entries.retain(|_, slot| {
                Arc::strong_count(slot) > 1
                    || slot.try_lock().map_or(true, |entry| {
                        entry
                            .as_ref()
                            .is_some_and(|(_, token)| reusable(token.expires_at, Utc::now()))
                    })
            });
            entries.entry(row.id).or_default().clone()
        };
        let mut cached = slot.lock().await;
        let generation = self.generation.load(Ordering::SeqCst);
        let revision = format!("{generation}:{}", revision(row));
        if !force_refresh
            && let Some((cached_revision, token)) = cached.as_ref()
            && cached_revision == &revision
            && reusable(token.expires_at, Utc::now())
        {
            return Ok(token.clone());
        }
        let result = mint().await?;
        if generation != self.generation.load(Ordering::SeqCst) {
            return Err(AppFailure::at(
                &AppHealth::default(),
                "Credential settings changed while minting; retry the operation",
            ));
        }
        *cached = Some((revision, result.clone()));
        Ok(result)
    }
}
pub async fn resolve(
    row: &GitHubOwnerTokenRow,
    app: &AppConfig,
) -> Result<InstallationToken, AppFailure> {
    resolve_with_refresh(row, app, false).await
}
async fn resolve_with_refresh(
    row: &GitHubOwnerTokenRow,
    app: &AppConfig,
    force_refresh: bool,
) -> Result<InstallationToken, AppFailure> {
    CACHE
        .get_or_mint(row, force_refresh, || async {
            let key =
                super::github_credentials::resolve_reference(app.private_key_reference.clone())
                    .await
                    .map_err(|e| AppFailure::at(&AppHealth::default(), e.to_string()))?;
            mint(&row.owner, app, &key, "https://api.github.com").await
        })
        .await
}
/// Explicit Settings probes verify current permissions/key access, not a
/// historical successful mint. They share the credential's mint lock.
pub async fn check_health(
    row: &GitHubOwnerTokenRow,
    app: &AppConfig,
) -> Result<InstallationToken, AppFailure> {
    resolve_with_refresh(row, app, true).await
}
fn reusable(expires_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    expires_at.timestamp() - now.timestamp() > MARGIN
}
fn missing_permissions(permissions: &HashMap<String, String>) -> Vec<String> {
    [
        ("metadata", "read"),
        ("contents", "write"),
        ("pull_requests", "write"),
        ("checks", "read"),
        ("statuses", "read"),
        ("actions", "read"),
    ]
    .into_iter()
    .filter(|(name, required)| {
        !permissions
            .get(*name)
            .is_some_and(|actual| actual == "write" || (*required == "read" && actual == "read"))
    })
    .map(|(name, level)| format!("{name}: {level}"))
    .collect()
}
#[derive(Serialize)]
struct Claims<'a> {
    iat: i64,
    exp: i64,
    iss: &'a str,
}
#[derive(Deserialize)]
struct AppIdentity {
    id: u64,
}
#[derive(Deserialize)]
struct Account {
    login: String,
}
#[derive(Deserialize)]
struct Installation {
    id: u64,
    app_id: u64,
    account: Account,
    suspended_at: Option<String>,
}
#[derive(Deserialize)]
struct Minted {
    token: String,
    expires_at: DateTime<Utc>,
    permissions: HashMap<String, String>,
}

async fn response<T: serde::de::DeserializeOwned>(
    request: reqwest::RequestBuilder,
    health: &AppHealth,
    stage: &str,
) -> Result<T, AppFailure> {
    let response = request.send().await.map_err(|_| {
        AppFailure::at(
            health,
            format!("{stage}: GitHub could not be reached; retry later"),
        )
    })?;
    let status = response.status();
    if !status.is_success() {
        return Err(AppFailure::at(
            health,
            format!(
                "{stage}: GitHub returned HTTP {}; check the App, installation and permissions",
                status.as_u16()
            ),
        ));
    }
    // Never echo GitHub response bodies, serde errors, JWTs or key material.
    response.json().await.map_err(|_| {
        AppFailure::at(
            health,
            format!("{stage}: GitHub returned an invalid response"),
        )
    })
}
async fn mint(
    owner: &str,
    app: &AppConfig,
    key: &str,
    api: &str,
) -> Result<InstallationToken, AppFailure> {
    let mut health = AppHealth::default();
    let key = jsonwebtoken::EncodingKey::from_rsa_pem(key.as_bytes()).map_err(|_| {
        AppFailure::at(
            &health,
            "The private-key reference must resolve to an RSA PEM key",
        )
    })?;
    let now = Utc::now().timestamp();
    let jwt = jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256),
        &Claims {
            iat: now - 60,
            exp: now + 540,
            iss: &app.app_id,
        },
        &key,
    )
    .map_err(|_| {
        AppFailure::at(
            &health,
            "The App JWT could not be signed; check the RSA private key",
        )
    })?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("vibe-kanban-github-app")
        .build()
        .map_err(|_| AppFailure::at(&health, "Could not initialize GitHub authentication"))?;
    let auth = |request: reqwest::RequestBuilder| {
        request
            .bearer_auth(&jwt)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
    };
    let identity: AppIdentity = response(
        auth(client.get(format!("{api}/app"))),
        &health,
        "App lookup",
    )
    .await?;
    if identity.id.to_string() != app.app_id {
        return Err(AppFailure::at(
            &health,
            "The key does not belong to the configured App ID",
        ));
    }
    health.app_reachable = true;
    let installation: Installation = response(
        auth(client.get(format!("{api}/app/installations/{}", app.installation_id))),
        &health,
        "Installation lookup",
    )
    .await?;
    if installation.id.to_string() != app.installation_id
        || installation.app_id != identity.id
        || !installation.account.login.eq_ignore_ascii_case(owner)
        || installation.suspended_at.is_some()
    {
        return Err(AppFailure::at(
            &health,
            "The installation must belong to this App and configured owner and must not be suspended",
        ));
    }
    health.installation_found = true;
    let minted: Minted = response(
        auth(client.post(format!(
            "{api}/app/installations/{}/access_tokens",
            app.installation_id
        )))
        .json(&serde_json::json!({})),
        &health,
        "Token mint",
    )
    .await?;
    if minted.token.is_empty()
        || minted
            .token
            .chars()
            .any(|c| c.is_whitespace() || c.is_control())
        || !reusable(minted.expires_at, Utc::now())
    {
        return Err(AppFailure::at(
            &health,
            "GitHub returned an empty or nearly expired installation token",
        ));
    }
    health.token_minted = true;
    health.missing_permissions = missing_permissions(&minted.permissions);
    Ok(InstallationToken {
        token: GitHubToken::new(minted.token),
        expires_at: minted.expires_at,
        health,
    })
}

const GRANT_AAD: &[u8] = b"vk-github-app-refresh-v1";
pub const BROKER_URL_ENV: &str = "VK_GITHUB_REFRESH_URL";
#[derive(Serialize, Deserialize)]
struct Grant {
    row_id: uuid::Uuid,
    revision: String,
    expires: i64,
}
fn revision(row: &GitHubOwnerTokenRow) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(row.encrypted_value.as_bytes()))
}
pub fn grant(row: &GitHubOwnerTokenRow) -> Result<String, GitHubOwnerTokenError> {
    grant_with(&super::github_owner_tokens::load_store()?, row)
}
fn grant_with(
    store: &super::mcp_gateway_secrets::McpGatewaySecretStore,
    row: &GitHubOwnerTokenRow,
) -> Result<String, GitHubOwnerTokenError> {
    let payload = serde_json::to_vec(&Grant {
        row_id: row.id,
        revision: revision(row),
        expires: Utc::now().timestamp() + 7 * 86400,
    })
    .map_err(|_| GitHubOwnerTokenError::App("Could not issue refresh authorization".into()))?;
    store
        .encrypt(&payload, GRANT_AAD)
        .map_err(|_| GitHubOwnerTokenError::KeyUnavailable)
}
fn refresh_refused() -> String {
    "GitHub App refresh authorization is invalid, expired or changed; start a new session".into()
}
async fn authorize_refresh(
    pool: &sqlx::SqlitePool,
    store: &super::mcp_gateway_secrets::McpGatewaySecretStore,
    capability: &str,
) -> Result<(GitHubOwnerTokenRow, Grant), String> {
    if capability.len() > 8192 {
        return Err(refresh_refused());
    }
    let payload = store
        .decrypt(capability, GRANT_AAD)
        .map_err(|_| refresh_refused())?;
    let grant: Grant = serde_json::from_slice(&payload).map_err(|_| refresh_refused())?;
    if grant.expires <= Utc::now().timestamp() {
        return Err(refresh_refused());
    }
    let row = GitHubOwnerTokenRow::find_by_id(pool, grant.row_id)
        .await
        .map_err(|_| refresh_refused())?
        .ok_or_else(refresh_refused)?;
    if grant.revision != revision(&row) {
        return Err(refresh_refused());
    }
    Ok((row, grant))
}
/// The opaque capability authorizes only its original row and revision.
pub async fn refresh(pool: &sqlx::SqlitePool, capability: &str) -> Result<GitHubToken, String> {
    let refused = refresh_refused;
    let store = super::github_owner_tokens::load_store().map_err(|_| refused())?;
    let (row, grant) = authorize_refresh(pool, &store, capability).await?;
    let value = super::github_owner_tokens::decrypt(&store, &row).map_err(|_| refused())?;
    let app = parse_config(&value).ok_or_else(refused)?;
    let result = resolve(&row, &app)
        .await
        .map_err(|e| format!("GitHub App for {}: {}", row.owner, e.detail))?;
    // Recheck after minting so an in-flight settings update cannot grant a token.
    let current = GitHubOwnerTokenRow::find_by_id(pool, row.id)
        .await
        .map_err(|_| refused())?
        .ok_or_else(refused)?;
    if revision(&current) != grant.revision {
        return Err(refused());
    }
    Ok(result.token)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_and_roundtrips_config_without_accepting_a_pat() {
        let app = AppConfig {
            app_id: "123".into(),
            installation_id: "456".into(),
            private_key_reference: "op://Vault/App/key".into(),
        };
        assert_eq!(
            parse_config(&encode_config(app.clone(), "").unwrap()),
            Some(app.clone())
        );
        assert!(encode_config(app.clone(), "synthetic-pat").is_err());
        for id in ["0", "-1", "1/../2", "18446744073709551616", ""] {
            assert!(
                encode_config(
                    AppConfig {
                        app_id: id.into(),
                        ..app.clone()
                    },
                    ""
                )
                .is_err()
            );
        }
        assert!(
            encode_config(
                AppConfig {
                    private_key_reference: "PEM secret".into(),
                    ..app
                },
                ""
            )
            .is_err()
        );
        assert!(parse_config("ghp_legacy").is_none());
    }
    #[test]
    fn expiry_margin_and_permissions_are_effective_not_assumed() {
        let now = Utc::now();
        assert!(!reusable(now + chrono::Duration::seconds(300), now));
        assert!(reusable(now + chrono::Duration::seconds(301), now));
        let mut permissions = HashMap::new();
        assert_eq!(missing_permissions(&permissions).len(), 6);
        for name in [
            "metadata",
            "contents",
            "pull_requests",
            "checks",
            "statuses",
            "actions",
        ] {
            permissions.insert(name.to_owned(), "read".to_owned());
        }
        assert_eq!(
            missing_permissions(&permissions),
            ["contents: write", "pull_requests: write"]
        );
    }
    #[tokio::test]
    async fn malformed_private_key_is_never_echoed() {
        let app = AppConfig {
            app_id: "1".into(),
            installation_id: "2".into(),
            private_key_reference: "op://v/i/f".into(),
        };
        let error = mint(
            "owner",
            &app,
            "private-synthetic-canary",
            "http://127.0.0.1:1",
        )
        .await
        .err()
        .unwrap();
        assert!(!format!("{error:?}").contains("private-synthetic-canary"));
        assert!(!error.health.app_reachable);
    }
    fn row() -> GitHubOwnerTokenRow {
        GitHubOwnerTokenRow {
            id: uuid::Uuid::new_v4(),
            owner: "owner".into(),
            encrypted_value: "synthetic revision".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
    fn token(value: &str) -> InstallationToken {
        InstallationToken {
            token: GitHubToken::new(value),
            expires_at: Utc::now() + chrono::Duration::hours(1),
            health: AppHealth::default(),
        }
    }
    #[tokio::test]
    async fn cache_coalesces_refresh_and_honors_revision_expiry_and_invalidation() {
        let cache = TokenCache::default();
        let mut row = row();
        let count = AtomicU64::new(0);
        let make = || async {
            count.fetch_add(1, Ordering::SeqCst);
            tokio::task::yield_now().await;
            Ok(token("synthetic"))
        };
        let (a, b) = tokio::join!(
            cache.get_or_mint(&row, false, make),
            cache.get_or_mint(&row, false, make)
        );
        assert!(a.is_ok() && b.is_ok());
        assert_eq!(count.load(Ordering::SeqCst), 1);
        row.encrypted_value = "rotated".into();
        cache.get_or_mint(&row, false, make).await.unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 2);
        let slot = cache.entries.lock().await.get(&row.id).unwrap().clone();
        slot.lock().await.as_mut().unwrap().1.expires_at = Utc::now();
        drop(slot);
        cache.get_or_mint(&row, false, make).await.unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 3);
        cache.generation.fetch_add(1, Ordering::SeqCst);
        let failed = cache
            .get_or_mint(&row, false, || async {
                cache.generation.fetch_add(1, Ordering::SeqCst);
                Ok(token("stale"))
            })
            .await;
        assert!(failed.is_err());
        cache.get_or_mint(&row, false, make).await.unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 4);
    }

    async fn fake_api(owner: &str, fail_mint: bool) -> (String, tokio::task::JoinHandle<()>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let owner = owner.to_owned();
        let task = tokio::spawn(async move {
            let count = if owner == "owner" { 3 } else { 2 };
            for stage in 0..count {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = vec![0; 8192];
                let n = socket.read(&mut bytes).await.unwrap();
                let request = String::from_utf8_lossy(&bytes[..n]);
                assert!(
                    request
                        .to_ascii_lowercase()
                        .contains("authorization: bearer ")
                );
                let expected = [
                    "GET /app ",
                    "GET /app/installations/2 ",
                    "POST /app/installations/2/access_tokens ",
                ][stage];
                assert!(request.starts_with(expected), "wrong API route");
                let body = match stage {
                    0 => serde_json::json!({"id": 1}),
                    1 => serde_json::json!({"id": 2, "app_id": 1, "account": {"login": owner}, "suspended_at": null}),
                    _ if fail_mint => serde_json::json!({"message": "secret-body-canary"}),
                    _ => serde_json::json!({"token": "synthetic-installation-token", "expires_at": Utc::now() + chrono::Duration::hours(1), "permissions": {"metadata":"read", "contents":"write", "pull_requests":"write", "checks":"read", "statuses":"read", "actions":"read"}}),
                }.to_string();
                let status = if stage == 2 && fail_mint {
                    "403 Forbidden"
                } else {
                    "200 OK"
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            }
        });
        (url, task)
    }
    #[tokio::test]
    async fn app_flow_checks_owner_and_reports_staged_safe_failures() {
        let app = AppConfig {
            app_id: "1".into(),
            installation_id: "2".into(),
            private_key_reference: "op://v/i/f".into(),
        };
        let pem = include_str!("testdata/github-app-test.pem");
        let (url, server) = fake_api("owner", false).await;
        let result = mint("OWNER", &app, pem, &url).await.unwrap();
        server.await.unwrap();
        assert!(
            result.health.app_reachable
                && result.health.installation_found
                && result.health.token_minted
        );
        assert!(result.health.missing_permissions.is_empty());
        assert_eq!(result.token.expose(), "synthetic-installation-token");
        let (url, server) = fake_api("wrong-owner", false).await;
        let error = mint("owner", &app, pem, &url).await.err().unwrap();
        server.await.unwrap();
        assert!(error.health.app_reachable && !error.health.installation_found);
        let (url, server) = fake_api("owner", true).await;
        let error = mint("owner", &app, pem, &url).await.err().unwrap();
        server.await.unwrap();
        assert!(error.health.installation_found && !error.health.token_minted);
        assert!(error.detail.contains("403"));
        assert!(!format!("{error:?}").contains("secret-body-canary"));
    }

    #[test]
    fn refresh_grants_are_separate_from_storage_and_bound_to_revision() {
        let directory = tempfile::tempdir().unwrap();
        let store = super::super::mcp_gateway_secrets::McpGatewaySecretStore::load_or_generate(
            &directory.path().join("key"),
        )
        .unwrap();
        let mut row = row();
        let capability = grant_with(&store, &row).unwrap();
        let bytes = store.decrypt(&capability, GRANT_AAD).unwrap();
        let claim: Grant = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(claim.row_id, row.id);
        assert_eq!(claim.revision, revision(&row));
        assert!(claim.expires > Utc::now().timestamp());
        assert!(
            store
                .decrypt(
                    &capability,
                    &super::super::github_owner_tokens::binding(row.id)
                )
                .is_err()
        );
        assert!(!capability.contains("synthetic"));
        row.encrypted_value = "changed".into();
        assert_ne!(claim.revision, revision(&row));
    }
    #[tokio::test]
    async fn broker_rejects_forged_expired_changed_and_deleted_grants() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
        let dir = tempfile::tempdir().unwrap();
        let store = super::super::mcp_gateway_secrets::McpGatewaySecretStore::load_or_generate(
            &dir.path().join("key"),
        )
        .unwrap();
        let row =
            GitHubOwnerTokenRow::create(&pool, uuid::Uuid::new_v4(), "owner", "synthetic-revision")
                .await
                .unwrap();
        let capability = grant_with(&store, &row).unwrap();
        let (_, mut grant) = authorize_refresh(&pool, &store, &capability).await.unwrap();
        assert!(authorize_refresh(&pool, &store, "forged").await.is_err());
        grant.expires = Utc::now().timestamp() - 1;
        let expired = store
            .encrypt(&serde_json::to_vec(&grant).unwrap(), GRANT_AAD)
            .unwrap();
        assert!(authorize_refresh(&pool, &store, &expired).await.is_err());
        let changed = GitHubOwnerTokenRow::update_value(&pool, row.id, "changed-revision")
            .await
            .unwrap()
            .unwrap();
        assert!(authorize_refresh(&pool, &store, &capability).await.is_err());
        let replacement = grant_with(&store, &changed).unwrap();
        assert!(authorize_refresh(&pool, &store, &replacement).await.is_ok());
        GitHubOwnerTokenRow::delete(&pool, row.id).await.unwrap();
        assert!(
            authorize_refresh(&pool, &store, &replacement)
                .await
                .is_err()
        );
    }
    #[tokio::test]
    async fn slow_owner_does_not_block_other_cached_or_uncached_owners() {
        let cache = Arc::new(TokenCache::default());
        let ready = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let cached_row = row();
        cache
            .get_or_mint(&cached_row, false, || async { Ok(token("cached")) })
            .await
            .unwrap();
        let stalled = {
            let cache = cache.clone();
            let ready = ready.clone();
            let release = release.clone();
            tokio::spawn(async move {
                cache
                    .get_or_mint(&row(), false, || async {
                        ready.notify_one();
                        release.notified().await;
                        Ok(token("slow"))
                    })
                    .await
            })
        };
        ready.notified().await;
        let result = tokio::time::timeout(Duration::from_secs(1), async {
            let cached = cache
                .get_or_mint(&cached_row, false, || async {
                    panic!("cached owner should not mint")
                })
                .await
                .unwrap();
            assert_eq!(cached.token.expose(), "cached");
            cache
                .get_or_mint(&row(), false, || async { Ok(token("other")) })
                .await
                .unwrap();
        })
        .await;
        release.notify_one();
        stalled.await.unwrap().unwrap();
        assert!(
            result.is_ok(),
            "one slow owner blocked unrelated credentials"
        );
    }
}
