//! Org-token credentials for the server's own GitHub operations.
//!
//! Agent processes receive Settings → Repositories → GitHub organization
//! tokens through their launch environment
//! ([`github_owner_tokens::launch_environment`]). The server's own git and
//! `gh` calls (create/merge PRs, pushes, fetches, PR polling) resolve them
//! here instead, **per request**: the table is re-read on every call so an
//! edit applies to the next operation, and `op://` references are resolved at
//! call time behind a short cache that Settings writes clear.
//!
//! The result is a [`GitHubCredentials`] set covering the owners a request
//! may contact. Owners without a row are absent (the server's existing
//! credential is used); a configured owner whose token cannot be read is
//! `Unavailable` and its commands fail closed. Values are never logged.

use std::{
    collections::{HashMap, HashSet},
    future::Future,
    path::Path,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use api_types::normalize_secret_reference;
use chrono::{DateTime, Utc};
use db::models::github_owner_token::GitHubOwnerTokenRow;
use git::{GitCli, GitService};
use sqlx::SqlitePool;
use utils::github_credentials::{
    GitHubCredentials, GitHubToken, OwnerCredential, parse_github_repo,
};
use uuid::Uuid;

use super::{
    environment_secrets::{EnvironmentSecretError, resolve_environment_secrets},
    github_owner_tokens::{GitHubOwnerTokenError, decrypt, load_store},
    mcp_gateway_secrets::McpGatewaySecretStore,
};

/// How long a resolved `op://` reference is reused. Settings writes clear the
/// cache, so this only bounds an in-place 1Password rotation behind an
/// unchanged reference.
const OP_CACHE_TTL: Duration = Duration::from_secs(60);
const RESOLVED_KEY: &str = "VALUE";

static OP_CACHE: LazyLock<OpCache> = LazyLock::new(OpCache::default);
/// Last credential source logged per owner, so polling logs changes only.
static LAST_SOURCE: LazyLock<Mutex<HashMap<String, &'static str>>> =
    LazyLock::new(Default::default);

/// Credentials for every GitHub owner named by `urls` (remote or PR URLs).
pub async fn resolve_for_urls<'a>(
    pool: &SqlitePool,
    urls: impl IntoIterator<Item = &'a str>,
) -> GitHubCredentials {
    let owners = owners_of(urls);
    if owners.is_empty() {
        return GitHubCredentials::default();
    }
    let rows = GitHubOwnerTokenRow::list(pool).await;
    resolve_rows(&owners, rows, load_store, resolve_reference, &OP_CACHE).await
}

/// Credentials for every remote of the repository at `repo_path`, so whichever
/// remote an operation contacts (default, push or an `upstream` base) is
/// covered.
pub async fn resolve_for_repo(
    pool: &SqlitePool,
    git: &GitService,
    repo_path: &Path,
) -> GitHubCredentials {
    let mut urls: Vec<String> = match git.list_remotes(repo_path) {
        Ok(remotes) => remotes.into_iter().map(|remote| remote.url).collect(),
        Err(error) => {
            tracing::warn!(
                repo = %repo_path.display(),
                %error,
                "Could not list remotes to select GitHub credentials; using the server credential"
            );
            Vec::new()
        }
    };
    // `git remote -v` shows URLs after `insteadOf` rewriting; one rewritten to
    // an SSH host alias no longer names its GitHub owner, while libgit2 and
    // git itself still contact the configured URL. Read those too.
    if let Ok(raw) = GitCli::new().git(
        repo_path,
        ["config", "--get-regexp", r"^remote\..*\.(url|pushurl)$"],
    ) {
        urls.extend(
            raw.lines()
                .filter_map(|line| line.split_whitespace().nth(1).map(str::to_string)),
        );
    }
    resolve_for_urls(pool, urls.iter().map(String::as_str)).await
}

/// Forget resolved `op://` values. Called on every Settings create, update
/// and delete, so a re-saved reference is read again on the next operation.
pub fn invalidate_cache() {
    OP_CACHE.invalidate();
}

fn owners_of<'a>(urls: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut seen = HashSet::new();
    urls.into_iter()
        .filter_map(parse_github_repo)
        .map(|target| target.owner)
        .filter(|owner| seen.insert(owner.to_ascii_lowercase()))
        .collect()
}

async fn resolve_reference(reference: String) -> Result<String, EnvironmentSecretError> {
    let mut resolved =
        resolve_environment_secrets(HashMap::from([(RESOLVED_KEY.to_owned(), reference)])).await?;
    Ok(resolved.remove(RESOLVED_KEY).unwrap_or_default())
}

async fn resolve_rows<L, R, F>(
    owners: &[String],
    rows: Result<Vec<GitHubOwnerTokenRow>, sqlx::Error>,
    load: L,
    resolve: R,
    cache: &OpCache,
) -> GitHubCredentials
where
    L: FnOnce() -> Result<McpGatewaySecretStore, GitHubOwnerTokenError>,
    R: Fn(String) -> F,
    F: Future<Output = Result<String, EnvironmentSecretError>>,
{
    let mut credentials = GitHubCredentials::default();
    let rows = match rows {
        Ok(rows) => rows,
        Err(error) => {
            // Whether any owner is configured is unknown: fail closed rather
            // than silently use another identity.
            tracing::error!(%error, "GitHub organization token table is unreadable");
            for owner in owners {
                credentials.insert(
                    owner,
                    OwnerCredential::Unavailable(
                        "the GitHub organization token table could not be read".into(),
                    ),
                );
            }
            log_sources(owners, &credentials);
            return credentials;
        }
    };
    let wanted: HashSet<String> = owners.iter().map(|o| o.to_ascii_lowercase()).collect();
    let matched: Vec<GitHubOwnerTokenRow> = rows
        .into_iter()
        .filter(|row| wanted.contains(&row.owner.to_ascii_lowercase()))
        .collect();
    if !matched.is_empty() {
        // The key file is touched only when a requested owner is configured.
        match load() {
            Ok(store) => {
                for row in &matched {
                    let credential = resolve_row(&store, row, &resolve, cache).await;
                    credentials.insert(&row.owner, credential);
                }
            }
            Err(error) => {
                for row in &matched {
                    credentials.insert(&row.owner, OwnerCredential::Unavailable(error.to_string()));
                }
            }
        }
    }
    log_sources(owners, &credentials);
    credentials
}

async fn resolve_row<R, F>(
    store: &McpGatewaySecretStore,
    row: &GitHubOwnerTokenRow,
    resolve: &R,
    cache: &OpCache,
) -> OwnerCredential
where
    R: Fn(String) -> F,
    F: Future<Output = Result<String, EnvironmentSecretError>>,
{
    let value = match decrypt(store, row) {
        Ok(value) => value,
        Err(error) => return OwnerCredential::Unavailable(error.to_string()),
    };
    let token = match normalize_secret_reference(&value) {
        None => value,
        Some(reference) => {
            let key = CacheKey {
                id: row.id,
                updated_at: row.updated_at,
                reference: reference.clone(),
            };
            if let Some(token) = cache.get(&key, Instant::now()) {
                return OwnerCredential::OrgToken(token);
            }
            let generation = cache.generation();
            match resolve(reference).await {
                Ok(resolved) => {
                    let resolved = resolved.trim().to_owned();
                    if !resolved.is_empty() {
                        cache.insert(key, GitHubToken::new(resolved.as_str()), generation);
                    }
                    resolved
                }
                Err(error) => return OwnerCredential::Unavailable(error.to_string()),
            }
        }
    };
    let token = token.trim();
    if token.is_empty() {
        return OwnerCredential::Unavailable(EnvironmentSecretError::InvalidValue.to_string());
    }
    OwnerCredential::OrgToken(GitHubToken::new(token))
}

/// Log the source each owner will use: `info` when it changed since last
/// logged, `debug` otherwise (the PR monitor resolves every minute).
fn log_sources(owners: &[String], credentials: &GitHubCredentials) {
    let mut last = LAST_SOURCE.lock().unwrap_or_else(|e| e.into_inner());
    for owner in owners {
        let source = credentials.select_owner(Some(owner)).source_label();
        let changed = last.insert(owner.to_ascii_lowercase(), source) != Some(source);
        if changed {
            tracing::info!(owner = %owner, source, "GitHub credential source for server operations");
        } else {
            tracing::debug!(owner = %owner, source, "GitHub credential source for server operations");
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    id: Uuid,
    updated_at: DateTime<Utc>,
    reference: String,
}

/// Resolved `op://` values. A generation counter keeps a lookup that was in
/// flight across an invalidation from re-inserting a superseded value.
#[derive(Default)]
struct OpCache {
    entries: Mutex<HashMap<CacheKey, (Instant, GitHubToken)>>,
    generation: AtomicU64,
}

impl OpCache {
    fn get(&self, key: &CacheKey, now: Instant) -> Option<GitHubToken> {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        match entries.get(key) {
            Some((at, token)) if now.saturating_duration_since(*at) < OP_CACHE_TTL => {
                Some(token.clone())
            }
            Some(_) => {
                entries.remove(key);
                None
            }
            None => None,
        }
    }

    fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    fn insert(&self, key: CacheKey, token: GitHubToken, generation: u64) {
        self.insert_at(key, token, generation, Instant::now());
    }

    fn insert_at(&self, key: CacheKey, token: GitHubToken, generation: u64, at: Instant) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        if self.generation() == generation {
            entries
                .retain(|_, (inserted, _)| at.saturating_duration_since(*inserted) < OP_CACHE_TTL);
            entries.insert(key, (at, token));
        }
    }

    fn invalidate(&self) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        self.generation.fetch_add(1, Ordering::SeqCst);
        entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::*;
    use crate::services::github_owner_tokens::binding;

    const OP_REF: &str = "op://Vault/GitHub sweetgreen/credential";

    fn temp_store() -> (tempfile::TempDir, McpGatewaySecretStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = McpGatewaySecretStore::load_or_generate(&dir.path().join("key")).unwrap();
        (dir, store)
    }

    fn row(store: &McpGatewaySecretStore, owner: &str, value: &str) -> GitHubOwnerTokenRow {
        let id = Uuid::new_v4();
        GitHubOwnerTokenRow {
            id,
            owner: owner.into(),
            encrypted_value: store.encrypt(value.as_bytes(), &binding(id)).unwrap(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn token_of(credentials: &GitHubCredentials, owner: &str) -> Option<String> {
        let selection = credentials.select_owner(Some(owner));
        selection.is_org_token().then(|| {
            // Read the token back the way a gh child would see it.
            let mut command = std::process::Command::new("true");
            selection.apply_gh(&mut command).unwrap();
            command
                .get_envs()
                .find(|(key, _)| *key == "GH_TOKEN")
                .and_then(|(_, value)| value)
                .map(|value| value.to_string_lossy().into_owned())
                .unwrap()
        })
    }

    fn counting_resolver(
        calls: Arc<AtomicUsize>,
        value: &'static str,
    ) -> impl Fn(String) -> std::future::Ready<Result<String, EnvironmentSecretError>> {
        move |reference: String| {
            assert_eq!(reference, OP_REF);
            calls.fetch_add(1, Ordering::SeqCst);
            std::future::ready(Ok(value.to_owned()))
        }
    }

    fn unused_resolver(_: String) -> std::future::Ready<Result<String, EnvironmentSecretError>> {
        panic!("literal tokens never reach the resolver")
    }

    #[test]
    fn owners_are_parsed_from_remote_and_pr_urls_and_deduplicated() {
        assert_eq!(
            owners_of([
                "git@github.com:Sweetgreen/platform-ops.git",
                "https://github.com/sweetgreen/other/pull/3",
                "https://github.com/davidvasandani/vibe-kanban",
                "https://dev.azure.com/org/project/_git/repo",
                "/srv/local/checkout",
            ]),
            vec!["Sweetgreen".to_string(), "davidvasandani".to_string()]
        );
    }

    #[tokio::test]
    async fn selects_by_owner_case_insensitively_and_falls_back_for_unknown_owners() {
        let (_dir, store) = temp_store();
        let rows = vec![
            row(&store, "sweetgreen", "synthetic-literal"),
            row(&store, "alderbridge", "synthetic-unrequested"),
        ];
        let credentials = resolve_rows(
            &["SweetGreen".into(), "someone".into()],
            Ok(rows),
            || Ok(store.clone()),
            unused_resolver,
            &OpCache::default(),
        )
        .await;
        assert_eq!(
            token_of(&credentials, "sweetgreen").as_deref(),
            Some("synthetic-literal")
        );
        assert_eq!(
            credentials.select_owner(Some("someone")).source_label(),
            "fallback"
        );
        // Owners the request does not touch are not resolved.
        assert_eq!(
            credentials.select_owner(Some("alderbridge")).source_label(),
            "fallback"
        );
    }

    #[tokio::test]
    async fn the_key_file_is_untouched_when_no_requested_owner_is_configured() {
        let (_dir, store) = temp_store();
        let credentials = resolve_rows(
            &["someone".into()],
            Ok(vec![row(&store, "sweetgreen", "synthetic")]),
            || panic!("the key store must not be loaded"),
            unused_resolver,
            &OpCache::default(),
        )
        .await;
        assert!(credentials.is_empty());
    }

    #[tokio::test]
    async fn unreadable_configured_tokens_fail_closed() {
        let (_dir, store) = temp_store();
        let (_other_dir, other_store) = temp_store();
        let failing =
            |_: String| std::future::ready(Err::<String, _>(EnvironmentSecretError::ReadFailed));
        // Wrong key (undecryptable), failed 1Password lookup, empty value.
        let credentials = resolve_rows(
            &["a".into(), "b".into(), "c".into()],
            Ok(vec![
                row(&other_store, "a", "synthetic"),
                row(&store, "b", OP_REF),
                row(&store, "c", "   "),
            ]),
            || Ok(store.clone()),
            failing,
            &OpCache::default(),
        )
        .await;
        for owner in ["a", "b", "c"] {
            assert_eq!(
                credentials.select_owner(Some(owner)).source_label(),
                "unavailable",
                "{owner}"
            );
        }
        let message = credentials.select_owner(Some("b")).attribute("HTTP 403");
        assert!(message.contains("1Password lookup failed"), "{message}");
        assert!(!message.contains("op://"), "{message}");

        let unreadable_table = resolve_rows(
            &["a".into()],
            Err(sqlx::Error::PoolClosed),
            || panic!("not reached"),
            unused_resolver,
            &OpCache::default(),
        )
        .await;
        assert_eq!(
            unreadable_table.select_owner(Some("a")).source_label(),
            "unavailable"
        );
        let missing_key = resolve_rows(
            &["a".into()],
            Ok(vec![row(&store, "a", "synthetic")]),
            || Err(GitHubOwnerTokenError::KeyUnavailable),
            unused_resolver,
            &OpCache::default(),
        )
        .await;
        assert_eq!(
            missing_key.select_owner(Some("a")).source_label(),
            "unavailable"
        );
    }

    #[tokio::test]
    async fn references_are_cached_until_ttl_edit_or_invalidation() {
        let (_dir, store) = temp_store();
        let cache = OpCache::default();
        let calls = Arc::new(AtomicUsize::new(0));
        let mut stored = row(&store, "sweetgreen", OP_REF);
        let owners = ["sweetgreen".to_string()];
        let resolve = |stored: &GitHubOwnerTokenRow| {
            resolve_rows(
                &owners,
                Ok(vec![stored.clone()]),
                || Ok(store.clone()),
                counting_resolver(calls.clone(), "synthetic-resolved\n"),
                &cache,
            )
        };

        let first = resolve(&stored).await;
        assert_eq!(
            token_of(&first, "sweetgreen").as_deref(),
            Some("synthetic-resolved")
        );
        resolve(&stored).await;
        assert_eq!(calls.load(Ordering::SeqCst), 1, "cached within the TTL");

        // Re-saving the row (new updated_at) misses the cache.
        stored.updated_at += chrono::Duration::milliseconds(1);
        resolve(&stored).await;
        assert_eq!(calls.load(Ordering::SeqCst), 2);

        // Settings writes invalidate everything.
        cache.invalidate();
        resolve(&stored).await;
        assert_eq!(calls.load(Ordering::SeqCst), 3);

        // An entry older than the TTL is re-read.
        let key = CacheKey {
            id: stored.id,
            updated_at: stored.updated_at,
            reference: OP_REF.into(),
        };
        let stale = Instant::now()
            .checked_sub(OP_CACHE_TTL + Duration::from_secs(1))
            .unwrap();
        cache.insert_at(
            key.clone(),
            GitHubToken::new("old"),
            cache.generation(),
            stale,
        );
        assert!(cache.get(&key, Instant::now()).is_none());
        resolve(&stored).await;
        assert_eq!(calls.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn a_lookup_spanning_an_invalidation_is_not_cached() {
        let cache = OpCache::default();
        let key = CacheKey {
            id: Uuid::new_v4(),
            updated_at: Utc::now(),
            reference: OP_REF.into(),
        };
        let generation = cache.generation();
        cache.invalidate();
        cache.insert(key.clone(), GitHubToken::new("superseded"), generation);
        assert!(cache.get(&key, Instant::now()).is_none());
    }
}
