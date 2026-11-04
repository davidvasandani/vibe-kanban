//! Per-owner GitHub tokens (Settings → Repositories → GitHub organization
//! tokens).
//!
//! Each configured GitHub owner maps to a PAT or App installation, stored as
//! a host-key envelope. Literal values are write-only: the API view exposes
//! only a 1Password reference, which is a pointer rather than a secret. At
//! launch, PATs are resolved and Apps receive scoped refresh grants consumed by
//! `utils::github_auth` on the spawning host. Errors never contain a value.

use std::{collections::HashMap, future::Future};

use api_types::normalize_secret_reference;
use chrono::{DateTime, Utc};
use db::models::github_owner_token::GitHubOwnerTokenRow;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use thiserror::Error;
use ts_rs::TS;
use utils::{
    assets::github_owner_tokens_key_path,
    github_auth::{OWNERS_ENV, is_valid_owner, token_env_name},
};
use uuid::Uuid;

use super::{
    environment_secrets::{
        CREDENTIAL_VARIABLES, EnvironmentSecretError, resolve_environment_secrets,
    },
    mcp_gateway_secrets::McpGatewaySecretStore,
};

const MAX_VALUE_BYTES: usize = 4 * 1024;
const RESOLVED_KEY: &str = "VALUE";

/// A configured owner as shown in settings. `reference` is present only when
/// the stored value is a 1Password reference; literal tokens are never
/// returned.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct GitHubOwnerToken {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub app: Option<super::github_app::AppConfig>,
    pub id: Uuid,
    pub owner: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reference: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateGitHubOwnerTokenRequest {
    pub owner: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    #[ts(optional)]
    pub app: Option<super::github_app::AppConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpdateGitHubOwnerTokenRequest {
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    #[ts(optional)]
    pub app: Option<super::github_app::AppConfig>,
}

#[derive(Debug, Error)]
pub enum GitHubOwnerTokenError {
    #[error(
        "GitHub owner must be 1-39 letters, digits or hyphens and cannot start or end with a hyphen"
    )]
    InvalidOwner,
    #[error("A token for GitHub owner {0} already exists; replace its value instead")]
    DuplicateOwner(String),
    #[error("GitHub token must be non-empty, at most 4 KiB, and contain no NUL bytes")]
    InvalidValue,
    #[error(
        "GitHub App configuration requires positive decimal IDs and a 1Password private-key reference, with no PAT value"
    )]
    InvalidApp,
    #[error("GitHub App credential: {0}")]
    App(String),
    #[error("GitHub owner token not found")]
    NotFound,
    #[error("The GitHub token encryption key is unavailable; check the Vibe Kanban data directory")]
    KeyUnavailable,
    #[error(
        "The stored GitHub token for {0} cannot be read; re-save it in Settings → Repositories"
    )]
    Undecryptable(String),
    #[error("GitHub token for {owner}: {source}")]
    Resolve {
        owner: String,
        #[source]
        source: EnvironmentSecretError,
    },
    #[error("GitHub token storage failed")]
    Database(#[from] sqlx::Error),
}

/// Normalize a submitted value: a 1Password reference loses copied quotes and
/// padding; a literal token is trimmed (GitHub tokens contain no whitespace).
fn normalize_value(value: &str) -> Result<String, GitHubOwnerTokenError> {
    let value = normalize_secret_reference(value).unwrap_or_else(|| value.trim().to_owned());
    if value.is_empty()
        || value.len() > MAX_VALUE_BYTES
        || value.contains('\0')
        || value.starts_with("vk-github-app-v1:")
    {
        return Err(GitHubOwnerTokenError::InvalidValue);
    }
    Ok(value)
}

fn validate_owner(owner: &str) -> Result<String, GitHubOwnerTokenError> {
    let owner = owner.trim();
    if is_valid_owner(owner) {
        Ok(owner.to_owned())
    } else {
        Err(GitHubOwnerTokenError::InvalidOwner)
    }
}

pub(crate) fn binding(id: Uuid) -> Vec<u8> {
    format!("vk-github-owner-token|{id}").into_bytes()
}

pub(crate) fn load_store() -> Result<McpGatewaySecretStore, GitHubOwnerTokenError> {
    McpGatewaySecretStore::load_or_generate(&github_owner_tokens_key_path())
        .map_err(|_| GitHubOwnerTokenError::KeyUnavailable)
}

pub(crate) fn decrypt(
    store: &McpGatewaySecretStore,
    row: &GitHubOwnerTokenRow,
) -> Result<String, GitHubOwnerTokenError> {
    store
        .decrypt(&row.encrypted_value, &binding(row.id))
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .ok_or_else(|| GitHubOwnerTokenError::Undecryptable(row.owner.clone()))
}

fn view(row: GitHubOwnerTokenRow, plaintext: Option<&str>) -> GitHubOwnerToken {
    GitHubOwnerToken {
        app: plaintext.and_then(super::github_app::parse_config),
        id: row.id,
        reference: plaintext.and_then(normalize_secret_reference),
        owner: row.owner,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

pub async fn list(pool: &SqlitePool) -> Result<Vec<GitHubOwnerToken>, GitHubOwnerTokenError> {
    let rows = GitHubOwnerTokenRow::list(pool).await?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    list_with(&load_store()?, rows)
}

fn list_with(
    store: &McpGatewaySecretStore,
    rows: Vec<GitHubOwnerTokenRow>,
) -> Result<Vec<GitHubOwnerToken>, GitHubOwnerTokenError> {
    Ok(rows
        .into_iter()
        .map(|row| {
            // An unreadable row is still listed so it can be replaced or
            // deleted; launches report it (fail closed).
            let plaintext = decrypt(store, &row).ok();
            view(row, plaintext.as_deref())
        })
        .collect())
}

pub async fn create(
    pool: &SqlitePool,
    request: CreateGitHubOwnerTokenRequest,
) -> Result<GitHubOwnerToken, GitHubOwnerTokenError> {
    let created = create_with(pool, &load_store()?, request).await;
    super::github_credentials::invalidate_cache();
    created
}

async fn create_with(
    pool: &SqlitePool,
    store: &McpGatewaySecretStore,
    request: CreateGitHubOwnerTokenRequest,
) -> Result<GitHubOwnerToken, GitHubOwnerTokenError> {
    let owner = validate_owner(&request.owner)?;
    let value = match request.app {
        Some(app) => super::github_app::encode_config(app, &request.value)?,
        None => normalize_value(&request.value)?,
    };
    let id = Uuid::new_v4();
    let envelope = store
        .encrypt(value.as_bytes(), &binding(id))
        .map_err(|_| GitHubOwnerTokenError::KeyUnavailable)?;
    let row = GitHubOwnerTokenRow::create(pool, id, &owner, &envelope)
        .await
        .map_err(|error| match &error {
            sqlx::Error::Database(db) if db.is_unique_violation() => {
                GitHubOwnerTokenError::DuplicateOwner(owner.clone())
            }
            _ => GitHubOwnerTokenError::Database(error),
        })?;
    Ok(view(row, Some(&value)))
}

pub async fn update(
    pool: &SqlitePool,
    id: Uuid,
    request: UpdateGitHubOwnerTokenRequest,
) -> Result<GitHubOwnerToken, GitHubOwnerTokenError> {
    let updated = update_with(pool, &load_store()?, id, request).await;
    super::github_credentials::invalidate_cache();
    updated
}

async fn update_with(
    pool: &SqlitePool,
    store: &McpGatewaySecretStore,
    id: Uuid,
    request: UpdateGitHubOwnerTokenRequest,
) -> Result<GitHubOwnerToken, GitHubOwnerTokenError> {
    let value = match request.app {
        Some(app) => super::github_app::encode_config(app, &request.value)?,
        None => normalize_value(&request.value)?,
    };
    let envelope = store
        .encrypt(value.as_bytes(), &binding(id))
        .map_err(|_| GitHubOwnerTokenError::KeyUnavailable)?;
    let row = GitHubOwnerTokenRow::update_value(pool, id, &envelope)
        .await?
        .ok_or(GitHubOwnerTokenError::NotFound)?;
    Ok(view(row, Some(&value)))
}

pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<(), GitHubOwnerTokenError> {
    super::github_credentials::invalidate_cache();
    if GitHubOwnerTokenRow::delete(pool, id).await? {
        Ok(())
    } else {
        Err(GitHubOwnerTokenError::NotFound)
    }
}

/// Environment for a workspace launch: [`OWNERS_ENV`] plus one resolved token
/// variable per owner, or an empty map when nothing is configured.
/// `org_env` is the unresolved organization Env Var map; its literal
/// `OP_SERVICE_ACCOUNT_TOKEN` and 1Password Connect settings select the
/// lookup credential, exactly as they do for Env Vars. Any configured owner that cannot be read or
/// resolved fails the launch.
pub async fn launch_environment(
    pool: &SqlitePool,
    org_env: &HashMap<String, String>,
) -> Result<HashMap<String, String>, GitHubOwnerTokenError> {
    let rows = GitHubOwnerTokenRow::list(pool).await?;
    if rows.is_empty() {
        return Ok(HashMap::new());
    }
    let store = load_store()?;
    let mut values = Vec::new();
    let mut app_environment = HashMap::new();
    let mut app_owners = Vec::new();
    for row in &rows {
        let value = decrypt(&store, row)?;
        if let Some(app) = super::github_app::parse_config(&value) {
            // Resolve at launch to report configuration failures early, but never
            // export the App key or a soon-to-expire installation token.
            super::github_app::resolve(row, &app).await.map_err(|e| {
                GitHubOwnerTokenError::App(format!("for {}: {}", row.owner, e.detail))
            })?;
            app_environment.insert(
                utils::github_auth::refresh_env_name(&row.owner),
                super::github_app::grant(row)?,
            );
            app_owners.push(row.owner.clone());
        } else {
            values.push((row.owner.clone(), value));
        }
    }
    let mut environment =
        launch_environment_with(values, org_env, resolve_environment_secrets).await?;
    if !app_owners.is_empty() {
        let mut owners = environment.remove(OWNERS_ENV).unwrap_or_default();
        if !owners.is_empty() {
            owners.push(',');
        }
        owners.push_str(&app_owners.join(","));
        environment.insert(OWNERS_ENV.to_owned(), owners);
        environment.extend(app_environment);
        let base = refresh_base_url(
            std::env::var("VIBE_BACKEND_URL").ok(),
            std::env::var("BACKEND_PORT")
                .or_else(|_| std::env::var("PORT"))
                .ok(),
            std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".into()),
            utils::port_file::read_port_file("vibe-kanban"),
        )
        .await?;
        environment.insert(
            super::github_app::BROKER_URL_ENV.into(),
            format!("{}/github-app-refresh", base.trim_end_matches('/')),
        );
    }
    Ok(environment)
}

async fn refresh_base_url(
    explicit: Option<String>,
    configured_port: Option<String>,
    host: String,
    bound_port: impl Future<Output = std::io::Result<u16>>,
) -> Result<String, GitHubOwnerTokenError> {
    if let Some(base) = explicit {
        return Ok(base);
    }
    let port = configured_port
        .map(|p| p.parse::<u16>())
        .transpose()
        .map_err(|_| GitHubOwnerTokenError::App("Invalid coordinator port".into()))?;
    // Zero asks TcpListener for an available port; it is never a destination.
    let port = match port.filter(|port| *port != 0) {
        Some(port) => port,
        None => bound_port.await.map_err(|_| {
            GitHubOwnerTokenError::App("Coordinator address unavailable for App refresh".into())
        })?,
    };
    if port == 0 {
        return Err(GitHubOwnerTokenError::App(
            "Coordinator bound port is unavailable".into(),
        ));
    }
    let host = match host.as_str() {
        "0.0.0.0" => "127.0.0.1",
        "::" | "[::]" => "[::1]",
        other => other,
    };
    Ok(format!("http://{host}:{port}"))
}

async fn launch_environment_with<R, F>(
    values: Vec<(String, String)>,
    org_env: &HashMap<String, String>,
    resolve: R,
) -> Result<HashMap<String, String>, GitHubOwnerTokenError>
where
    R: Fn(HashMap<String, String>) -> F,
    F: Future<Output = Result<HashMap<String, String>, EnvironmentSecretError>>,
{
    let mut environment = HashMap::new();
    let mut owners = Vec::with_capacity(values.len());
    for (owner, value) in values {
        let token = if normalize_secret_reference(&value).is_some() {
            let mut request = HashMap::from([(RESOLVED_KEY.to_owned(), value)]);
            for name in CREDENTIAL_VARIABLES {
                if let Some(credential) = org_env.get(name) {
                    request.insert(name.to_owned(), credential.clone());
                }
            }
            resolve(request)
                .await
                .map_err(|source| GitHubOwnerTokenError::Resolve {
                    owner: owner.clone(),
                    // The request key is a placeholder; the owner names the input.
                    source: source.without_variable(),
                })?
                .remove(RESOLVED_KEY)
                .unwrap_or_default()
        } else {
            value
        };
        // A reference that resolves to an empty or whitespace field would
        // otherwise be exported and then rejected by the shim at each call.
        let token = token.trim().to_owned();
        if token.is_empty() {
            return Err(GitHubOwnerTokenError::Resolve {
                owner,
                source: EnvironmentSecretError::InvalidValue,
            });
        }
        environment.insert(token_env_name(&owner), token);
        owners.push(owner);
    }
    environment.insert(OWNERS_ENV.to_owned(), owners.join(","));
    Ok(environment)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    const OP_TOKEN: &str = "OP_SERVICE_ACCOUNT_TOKEN";
    use crate::services::environment_secrets::{LookupFailure, Via};

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

    fn create_request(owner: &str, value: &str) -> CreateGitHubOwnerTokenRequest {
        CreateGitHubOwnerTokenRequest {
            owner: owner.into(),
            value: value.into(),
            app: None,
        }
    }

    #[test]
    fn values_normalize_references_and_trim_literals() {
        assert_eq!(
            normalize_value("\"op://Vault/GitHub PAT/credential\"").unwrap(),
            "op://Vault/GitHub PAT/credential"
        );
        assert_eq!(
            normalize_value("  synthetic_pat_value\n").unwrap(),
            "synthetic_pat_value"
        );
        for invalid in ["", "   ", "a\0b"] {
            assert!(normalize_value(invalid).is_err(), "{invalid:?}");
        }
        assert!(normalize_value(&"x".repeat(MAX_VALUE_BYTES + 1)).is_err());
        assert_eq!(validate_owner(" Org-A ").unwrap(), "Org-A");
        assert!(validate_owner("a_b").is_err());
    }

    #[tokio::test]
    async fn crud_encrypts_hides_literals_and_rejects_case_duplicates() {
        let pool = test_pool().await;
        let (_dir, store) = temp_store();
        let literal = create_with(&pool, &store, create_request("Org-A", "synthetic-literal"))
            .await
            .unwrap();
        assert_eq!(literal.reference, None);
        let reference = create_with(
            &pool,
            &store,
            create_request("org-b", "“op://Vault/org b/credential”"),
        )
        .await
        .unwrap();
        assert_eq!(
            reference.reference.as_deref(),
            Some("op://Vault/org b/credential")
        );
        let duplicate = create_with(&pool, &store, create_request("ORG-A", "x")).await;
        assert!(matches!(
            duplicate,
            Err(GitHubOwnerTokenError::DuplicateOwner(owner)) if owner == "ORG-A"
        ));

        let rows = GitHubOwnerTokenRow::list(&pool).await.unwrap();
        assert!(
            rows.iter()
                .all(|row| !row.encrypted_value.contains("synthetic"))
        );
        let listed = list_with(&store, rows).unwrap();
        assert_eq!(listed.len(), 2);
        let json = serde_json::to_string(&listed).unwrap();
        assert!(!json.contains("synthetic-literal"));
        assert!(json.contains("op://Vault/org b/credential"));

        let updated = update_with(
            &pool,
            &store,
            literal.id,
            UpdateGitHubOwnerTokenRequest {
                app: None,
                value: "op://Vault/a/credential".into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(
            updated.reference.as_deref(),
            Some("op://Vault/a/credential")
        );
        assert!(matches!(
            update_with(
                &pool,
                &store,
                Uuid::new_v4(),
                UpdateGitHubOwnerTokenRequest {
                    value: "x".into(),
                    app: None
                }
            )
            .await,
            Err(GitHubOwnerTokenError::NotFound)
        ));
        delete(&pool, reference.id).await.unwrap();
        assert!(matches!(
            delete(&pool, reference.id).await,
            Err(GitHubOwnerTokenError::NotFound)
        ));
    }

    #[tokio::test]
    async fn envelopes_are_bound_to_their_row() {
        let pool = test_pool().await;
        let (_dir, store) = temp_store();
        let a = create_with(&pool, &store, create_request("a", "synthetic-a"))
            .await
            .unwrap();
        let b = create_with(&pool, &store, create_request("b", "synthetic-b"))
            .await
            .unwrap();
        let row_a = GitHubOwnerTokenRow::find_by_id(&pool, a.id)
            .await
            .unwrap()
            .unwrap();
        let mut row_b = GitHubOwnerTokenRow::find_by_id(&pool, b.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(decrypt(&store, &row_a).unwrap(), "synthetic-a");
        row_b.encrypted_value = row_a.encrypted_value.clone();
        assert!(matches!(
            decrypt(&store, &row_b),
            Err(GitHubOwnerTokenError::Undecryptable(owner)) if owner == "b"
        ));
        let (_other_dir, other_store) = temp_store();
        assert!(decrypt(&other_store, &row_a).is_err());
    }

    #[tokio::test]
    async fn empty_configuration_adds_nothing() {
        let pool = test_pool().await;
        assert!(
            launch_environment(&pool, &HashMap::new())
                .await
                .unwrap()
                .is_empty()
        );
        assert!(list(&pool).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn launch_environment_resolves_references_with_org_token() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let resolver = {
            let seen = seen.clone();
            move |request: HashMap<String, String>| {
                seen.lock().unwrap().push(request.clone());
                async move {
                    Ok(HashMap::from([(
                        RESOLVED_KEY.to_owned(),
                        "synthetic-resolved\n".to_owned(),
                    )]))
                }
            }
        };
        let org = HashMap::from([
            (OP_TOKEN.to_owned(), "synthetic-op".to_owned()),
            (
                "OP_CONNECT_HOST".to_owned(),
                "http://connect:8080".to_owned(),
            ),
            (
                "OP_CONNECT_TOKEN".to_owned(),
                "synthetic-connect".to_owned(),
            ),
            ("UNRELATED".to_owned(), "never-forwarded".to_owned()),
        ]);
        let environment = launch_environment_with(
            vec![
                ("Org-A".into(), "synthetic-literal".into()),
                ("org-b".into(), "op://Vault/b/credential".into()),
            ],
            &org,
            resolver,
        )
        .await
        .unwrap();
        assert_eq!(environment[OWNERS_ENV], "Org-A,org-b");
        assert_eq!(environment[&token_env_name("Org-A")], "synthetic-literal");
        assert_eq!(environment[&token_env_name("org-b")], "synthetic-resolved");
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 1, "literals never reach the resolver");
        assert_eq!(seen[0][OP_TOKEN], "synthetic-op");
        assert_eq!(seen[0]["OP_CONNECT_HOST"], "http://connect:8080");
        assert_eq!(seen[0]["OP_CONNECT_TOKEN"], "synthetic-connect");
        assert!(!seen[0].contains_key("UNRELATED"));
        assert_eq!(seen[0][RESOLVED_KEY], "op://Vault/b/credential");
    }

    #[tokio::test]
    async fn an_owner_named_like_the_manifest_keeps_its_token() {
        let unused = |_request: HashMap<String, String>| async {
            Err(EnvironmentSecretError::ReadFailed {
                variable: Some(RESOLVED_KEY.to_owned()),
                via: Via::ServiceAccount,
                cause: LookupFailure::Unknown,
            })
        };
        let environment = launch_environment_with(
            vec![("owners".into(), "synthetic-owners".into())],
            &HashMap::new(),
            unused,
        )
        .await
        .unwrap();
        assert_eq!(environment[OWNERS_ENV], "owners");
        assert_eq!(environment[&token_env_name("owners")], "synthetic-owners");
    }

    #[tokio::test]
    async fn launch_environment_names_the_failing_owner() {
        let failing = |_request: HashMap<String, String>| async {
            Err(EnvironmentSecretError::ReadFailed {
                variable: Some(RESOLVED_KEY.to_owned()),
                via: Via::ServiceAccount,
                cause: LookupFailure::Unknown,
            })
        };
        let error = launch_environment_with(
            vec![("org-b".into(), "op://Vault/b/credential".into())],
            &HashMap::new(),
            failing,
        )
        .await
        .unwrap_err();
        let message = error.to_string();
        assert!(message.contains("org-b"), "{message}");
        assert!(message.contains("1Password lookup failed"), "{message}");
        assert!(!message.contains("op://"));
        assert!(!message.contains(RESOLVED_KEY), "{message}");

        let empty = |_request: HashMap<String, String>| async {
            Ok(HashMap::from([(RESOLVED_KEY.to_owned(), " ".to_owned())]))
        };
        assert!(
            launch_environment_with(
                vec![("a".into(), "op://Vault/a/credential".into())],
                &HashMap::new(),
                empty,
            )
            .await
            .is_err()
        );
    }
    #[tokio::test]
    async fn app_crud_preserves_row_identity_and_allows_pat_migration() {
        let pool = test_pool().await;
        let (_directory, store) = temp_store();
        let legacy: CreateGitHubOwnerTokenRequest =
            serde_json::from_str(r#"{"owner":"example","value":"synthetic-pat"}"#).unwrap();
        let pat = create_with(&pool, &store, legacy).await.unwrap();
        assert!(pat.app.is_none());
        let app = super::super::github_app::AppConfig {
            app_id: "1".into(),
            installation_id: "2".into(),
            private_key_reference: "op://Vault/App/key".into(),
        };
        let changed = update_with(
            &pool,
            &store,
            pat.id,
            UpdateGitHubOwnerTokenRequest {
                value: String::new(),
                app: Some(app.clone()),
            },
        )
        .await
        .unwrap();
        assert_eq!(changed.id, pat.id);
        assert_eq!(changed.app, Some(app));
        assert!(changed.reference.is_none());
        let row = GitHubOwnerTokenRow::find_by_id(&pool, pat.id)
            .await
            .unwrap()
            .unwrap();
        assert!(!row.encrypted_value.contains("op://"));
        let back = update_with(
            &pool,
            &store,
            pat.id,
            UpdateGitHubOwnerTokenRequest {
                value: "synthetic-new-pat".into(),
                app: None,
            },
        )
        .await
        .unwrap();
        assert!(back.app.is_none());
        assert!(
            !serde_json::to_string(&back)
                .unwrap()
                .contains("synthetic-new-pat")
        );
        assert!(normalize_value("vk-github-app-v1:{}").is_err());
    }
    #[tokio::test]
    async fn refresh_uses_actual_port_for_auto_assignment_and_preserves_bound_host() {
        for configured in [None, Some("0".into())] {
            let base = refresh_base_url(None, configured, "0.0.0.0".into(), async { Ok(45123) })
                .await
                .unwrap();
            assert_eq!(base, "http://127.0.0.1:45123");
        }
        let base = refresh_base_url(None, Some("3334".into()), "192.0.2.10".into(), async {
            panic!("explicit nonzero port does not read the port file")
        })
        .await
        .unwrap();
        assert_eq!(base, "http://192.0.2.10:3334");
        let base = refresh_base_url(
            Some("https://coordinator.example".into()),
            Some("0".into()),
            "127.0.0.1".into(),
            async { panic!("explicit base takes precedence") },
        )
        .await
        .unwrap();
        assert_eq!(base, "https://coordinator.example");
        assert!(
            refresh_base_url(None, Some("0".into()), "127.0.0.1".into(), async {
                Err(std::io::Error::other("unavailable"))
            })
            .await
            .is_err()
        );
    }
}
