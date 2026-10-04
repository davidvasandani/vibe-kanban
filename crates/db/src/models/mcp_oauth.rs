//! Persistence for the embedded OAuth 2.1 authorization server that admits
//! remote MCP clients (e.g. ChatGPT connectors). Callers hash every secret
//! before it reaches this module, and pass `now` explicitly so expiry logic is
//! deterministic under test.

use chrono::{DateTime, Duration, SecondsFormat, Utc};
use serde::Serialize;
use sqlx::{FromRow, SqlitePool};

/// Fixed-width UTC timestamp so stored values compare lexicographically.
pub fn timestamp(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(SecondsFormat::Micros, true)
}

#[derive(Clone, Debug, FromRow)]
pub struct McpOAuthClient {
    pub client_id: String,
    pub client_secret_hash: Option<Vec<u8>>,
    pub token_endpoint_auth_method: String,
    pub client_name: Option<String>,
    pub redirect_uris: String,
    pub created_at: String,
}

impl McpOAuthClient {
    pub fn redirect_uris(&self) -> Vec<String> {
        serde_json::from_str(&self.redirect_uris).unwrap_or_default()
    }
}

pub struct NewMcpOAuthClient<'a> {
    pub client_id: &'a str,
    pub client_secret_hash: Option<&'a [u8]>,
    pub token_endpoint_auth_method: &'a str,
    pub client_name: Option<&'a str>,
    pub redirect_uris: &'a [String],
}

#[derive(Clone, Debug, FromRow)]
pub struct McpOAuthAuthorization {
    pub id: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub code_challenge: String,
    pub scope: String,
    pub resource: String,
    pub state: Option<String>,
    pub consent_hash: Vec<u8>,
    pub status: String,
    pub grant_id: Option<String>,
    pub expires_at: String,
}

pub struct NewMcpOAuthAuthorization<'a> {
    pub id: &'a str,
    pub client_id: &'a str,
    pub redirect_uri: &'a str,
    pub code_challenge: &'a str,
    pub scope: &'a str,
    pub resource: &'a str,
    pub state: Option<&'a str>,
    pub consent_hash: &'a [u8],
    pub expires_at: DateTime<Utc>,
}

/// A token row joined with the grant it belongs to.
#[derive(Clone, Debug, FromRow)]
pub struct McpOAuthTokenRecord {
    pub grant_id: String,
    pub kind: String,
    pub expires_at: String,
    pub revoked_at: Option<String>,
    pub client_id: String,
    pub scope: String,
    pub resource: String,
    pub grant_revoked_at: Option<String>,
    pub last_used_at: Option<String>,
}

/// The plaintext-free description of a token pair to persist.
pub struct IssuedTokenHashes<'a> {
    pub access_hash: &'a [u8],
    pub access_expires_at: DateTime<Utc>,
    pub refresh_hash: &'a [u8],
    pub refresh_expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct McpOAuthGrantSummary {
    pub id: String,
    pub client_id: String,
    pub client_name: Option<String>,
    pub scope: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ExchangeOutcome {
    Issued {
        grant_id: String,
    },
    /// The code was not in the `approved` state any more (lost a race).
    NotApproved,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RotateOutcome {
    Rotated,
    /// The presented refresh token had already been rotated (lost a race or
    /// was replayed); the caller treats this as reuse.
    AlreadyRevoked,
}

/// Unclaimed client registrations are removed after this long.
pub const UNCLAIMED_CLIENT_TTL: Duration = Duration::hours(24);
/// Exchanged/expired authorization rows are kept this long after expiry so a
/// replayed code is still recognised and revokes its grant.
const AUTHORIZATION_RETENTION: Duration = Duration::days(1);

pub struct McpOAuth;

impl McpOAuth {
    /// Remove expired state: authorization rows past retention, expired
    /// tokens, grants left with no tokens or revoked long ago, and client
    /// registrations that never obtained a grant.
    pub async fn prune(pool: &SqlitePool, now: DateTime<Utc>) -> Result<(), sqlx::Error> {
        let now_ts = timestamp(now);
        let retention_ts = timestamp(now - AUTHORIZATION_RETENTION);
        let unclaimed_ts = timestamp(now - UNCLAIMED_CLIENT_TTL);
        let mut tx = pool.begin().await?;
        sqlx::query("DELETE FROM mcp_oauth_authorizations WHERE expires_at < ?")
            .bind(&retention_ts)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM mcp_oauth_tokens WHERE expires_at < ?")
            .bind(&now_ts)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            r#"DELETE FROM mcp_oauth_grants
               WHERE (revoked_at IS NOT NULL AND revoked_at < ?)
                  OR NOT EXISTS (SELECT 1 FROM mcp_oauth_tokens t WHERE t.grant_id = mcp_oauth_grants.id)"#,
        )
        .bind(&retention_ts)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"DELETE FROM mcp_oauth_clients
               WHERE created_at < ?
                 AND NOT EXISTS (SELECT 1 FROM mcp_oauth_grants g WHERE g.client_id = mcp_oauth_clients.client_id)
                 AND NOT EXISTS (SELECT 1 FROM mcp_oauth_authorizations a
                                 WHERE a.client_id = mcp_oauth_clients.client_id AND a.expires_at >= ?)"#,
        )
        .bind(&unclaimed_ts)
        .bind(&now_ts)
        .execute(&mut *tx)
        .await?;
        tx.commit().await
    }

    pub async fn count_clients(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar("SELECT COUNT(*) FROM mcp_oauth_clients")
            .fetch_one(pool)
            .await
    }

    pub async fn insert_client(
        pool: &SqlitePool,
        client: NewMcpOAuthClient<'_>,
        now: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        let redirect_uris = serde_json::to_string(client.redirect_uris)
            .map_err(|e| sqlx::Error::Encode(e.into()))?;
        sqlx::query(
            r#"INSERT INTO mcp_oauth_clients
                   (client_id, client_secret_hash, token_endpoint_auth_method, client_name,
                    redirect_uris, created_at)
               VALUES (?, ?, ?, ?, ?, ?)"#,
        )
        .bind(client.client_id)
        .bind(client.client_secret_hash)
        .bind(client.token_endpoint_auth_method)
        .bind(client.client_name)
        .bind(redirect_uris)
        .bind(timestamp(now))
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn find_client(
        pool: &SqlitePool,
        client_id: &str,
    ) -> Result<Option<McpOAuthClient>, sqlx::Error> {
        sqlx::query_as::<_, McpOAuthClient>(
            r#"SELECT client_id, client_secret_hash, token_endpoint_auth_method, client_name,
                      redirect_uris, created_at
               FROM mcp_oauth_clients WHERE client_id = ?"#,
        )
        .bind(client_id)
        .fetch_optional(pool)
        .await
    }

    pub async fn insert_authorization(
        pool: &SqlitePool,
        authorization: NewMcpOAuthAuthorization<'_>,
        now: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"INSERT INTO mcp_oauth_authorizations
                   (id, client_id, redirect_uri, code_challenge, scope, resource, state,
                    consent_hash, status, expires_at, created_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?, ?)"#,
        )
        .bind(authorization.id)
        .bind(authorization.client_id)
        .bind(authorization.redirect_uri)
        .bind(authorization.code_challenge)
        .bind(authorization.scope)
        .bind(authorization.resource)
        .bind(authorization.state)
        .bind(authorization.consent_hash)
        .bind(timestamp(authorization.expires_at))
        .bind(timestamp(now))
        .execute(pool)
        .await?;
        Ok(())
    }

    const AUTHORIZATION_COLUMNS: &'static str = "id, client_id, redirect_uri, code_challenge, \
         scope, resource, state, consent_hash, status, grant_id, expires_at";

    pub async fn find_authorization(
        pool: &SqlitePool,
        id: &str,
    ) -> Result<Option<McpOAuthAuthorization>, sqlx::Error> {
        sqlx::query_as::<_, McpOAuthAuthorization>(&format!(
            "SELECT {} FROM mcp_oauth_authorizations WHERE id = ?",
            Self::AUTHORIZATION_COLUMNS
        ))
        .bind(id)
        .fetch_optional(pool)
        .await
    }

    pub async fn find_authorization_by_code(
        pool: &SqlitePool,
        code_hash: &[u8],
    ) -> Result<Option<McpOAuthAuthorization>, sqlx::Error> {
        sqlx::query_as::<_, McpOAuthAuthorization>(&format!(
            "SELECT {} FROM mcp_oauth_authorizations WHERE code_hash = ?",
            Self::AUTHORIZATION_COLUMNS
        ))
        .bind(code_hash)
        .fetch_optional(pool)
        .await
    }

    /// Move a pending, unexpired authorization to `approved` (with its code)
    /// or `denied`. Returns false when it was no longer pending or expired, so
    /// a consent form can be used at most once.
    pub async fn resolve_pending(
        pool: &SqlitePool,
        id: &str,
        approved_code: Option<(&[u8], DateTime<Utc>)>,
        now: DateTime<Utc>,
    ) -> Result<bool, sqlx::Error> {
        let (status, code_hash, expires_at) = match approved_code {
            Some((code_hash, expires_at)) => {
                ("approved", Some(code_hash), Some(timestamp(expires_at)))
            }
            None => ("denied", None, None),
        };
        let result = sqlx::query(
            r#"UPDATE mcp_oauth_authorizations
               SET status = ?, code_hash = ?, expires_at = COALESCE(?, expires_at)
               WHERE id = ? AND status = 'pending' AND expires_at > ?"#,
        )
        .bind(status)
        .bind(code_hash)
        .bind(expires_at)
        .bind(id)
        .bind(timestamp(now))
        .execute(pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    /// Atomically mark an approved code exchanged and mint its grant and
    /// first token pair.
    pub async fn exchange_code(
        pool: &SqlitePool,
        authorization: &McpOAuthAuthorization,
        grant_id: &str,
        tokens: IssuedTokenHashes<'_>,
        now: DateTime<Utc>,
    ) -> Result<ExchangeOutcome, sqlx::Error> {
        let now_ts = timestamp(now);
        let mut tx = pool.begin().await?;
        sqlx::query(
            r#"INSERT INTO mcp_oauth_grants (id, client_id, scope, resource, created_at)
               VALUES (?, ?, ?, ?, ?)"#,
        )
        .bind(grant_id)
        .bind(&authorization.client_id)
        .bind(&authorization.scope)
        .bind(&authorization.resource)
        .bind(&now_ts)
        .execute(&mut *tx)
        .await?;
        let updated = sqlx::query(
            r#"UPDATE mcp_oauth_authorizations SET status = 'exchanged', grant_id = ?
               WHERE id = ? AND status = 'approved' AND expires_at > ?"#,
        )
        .bind(grant_id)
        .bind(&authorization.id)
        .bind(&now_ts)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            tx.rollback().await?;
            return Ok(ExchangeOutcome::NotApproved);
        }
        insert_token_pair(&mut tx, grant_id, &tokens, &now_ts).await?;
        tx.commit().await?;
        Ok(ExchangeOutcome::Issued {
            grant_id: grant_id.to_string(),
        })
    }

    pub async fn find_token(
        pool: &SqlitePool,
        token_hash: &[u8],
    ) -> Result<Option<McpOAuthTokenRecord>, sqlx::Error> {
        sqlx::query_as::<_, McpOAuthTokenRecord>(
            r#"SELECT t.grant_id, t.kind, t.expires_at, t.revoked_at,
                      g.client_id, g.scope, g.resource, g.revoked_at AS grant_revoked_at,
                      g.last_used_at
               FROM mcp_oauth_tokens t
               JOIN mcp_oauth_grants g ON g.id = t.grant_id
               WHERE t.token_hash = ?"#,
        )
        .bind(token_hash)
        .fetch_optional(pool)
        .await
    }

    /// Revoke `old_refresh_hash` and mint a new pair on the same grant, in one
    /// transaction.
    pub async fn rotate_refresh(
        pool: &SqlitePool,
        old_refresh_hash: &[u8],
        grant_id: &str,
        tokens: IssuedTokenHashes<'_>,
        now: DateTime<Utc>,
    ) -> Result<RotateOutcome, sqlx::Error> {
        let now_ts = timestamp(now);
        let mut tx = pool.begin().await?;
        let updated = sqlx::query(
            r#"UPDATE mcp_oauth_tokens SET revoked_at = ?
               WHERE token_hash = ? AND kind = 'refresh' AND revoked_at IS NULL"#,
        )
        .bind(&now_ts)
        .bind(old_refresh_hash)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            tx.rollback().await?;
            return Ok(RotateOutcome::AlreadyRevoked);
        }
        insert_token_pair(&mut tx, grant_id, &tokens, &now_ts).await?;
        tx.commit().await?;
        Ok(RotateOutcome::Rotated)
    }

    /// Revoke a grant and every token issued under it. Returns false when the
    /// grant does not exist or was already revoked.
    pub async fn revoke_grant(
        pool: &SqlitePool,
        grant_id: &str,
        now: DateTime<Utc>,
    ) -> Result<bool, sqlx::Error> {
        let now_ts = timestamp(now);
        let mut tx = pool.begin().await?;
        let updated = sqlx::query(
            "UPDATE mcp_oauth_grants SET revoked_at = ? WHERE id = ? AND revoked_at IS NULL",
        )
        .bind(&now_ts)
        .bind(grant_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE mcp_oauth_tokens SET revoked_at = ? WHERE grant_id = ? AND revoked_at IS NULL",
        )
        .bind(&now_ts)
        .bind(grant_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(updated.rows_affected() == 1)
    }

    /// Returns the grant id when `token_hash` is a live access token for
    /// `resource` on a live grant.
    pub async fn verify_access_token(
        pool: &SqlitePool,
        token_hash: &[u8],
        resource: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<McpOAuthTokenRecord>, sqlx::Error> {
        let Some(record) = Self::find_token(pool, token_hash).await? else {
            return Ok(None);
        };
        let now_ts = timestamp(now);
        let live = record.kind == "access"
            && record.revoked_at.is_none()
            && record.grant_revoked_at.is_none()
            && record.expires_at > now_ts
            && record.resource == resource;
        Ok(live.then_some(record))
    }

    /// Record grant use, at most once per `min_interval`, to keep verify cheap.
    pub async fn touch_grant(
        pool: &SqlitePool,
        grant_id: &str,
        now: DateTime<Utc>,
        min_interval: Duration,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"UPDATE mcp_oauth_grants SET last_used_at = ?
               WHERE id = ? AND (last_used_at IS NULL OR last_used_at < ?)"#,
        )
        .bind(timestamp(now))
        .bind(grant_id)
        .bind(timestamp(now - min_interval))
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn list_live_grants(
        pool: &SqlitePool,
    ) -> Result<Vec<McpOAuthGrantSummary>, sqlx::Error> {
        sqlx::query_as::<_, McpOAuthGrantSummary>(
            r#"SELECT g.id, g.client_id, c.client_name, g.scope, g.created_at, g.last_used_at
               FROM mcp_oauth_grants g
               JOIN mcp_oauth_clients c ON c.client_id = g.client_id
               WHERE g.revoked_at IS NULL
               ORDER BY g.created_at DESC"#,
        )
        .fetch_all(pool)
        .await
    }
}

async fn insert_token_pair(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    grant_id: &str,
    tokens: &IssuedTokenHashes<'_>,
    now_ts: &str,
) -> Result<(), sqlx::Error> {
    for (hash, kind, expires_at) in [
        (tokens.access_hash, "access", tokens.access_expires_at),
        (tokens.refresh_hash, "refresh", tokens.refresh_expires_at),
    ] {
        sqlx::query(
            r#"INSERT INTO mcp_oauth_tokens (token_hash, grant_id, kind, expires_at, created_at)
               VALUES (?, ?, ?, ?, ?)"#,
        )
        .bind(hash)
        .bind(grant_id)
        .bind(kind)
        .bind(timestamp(expires_at))
        .bind(now_ts)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;

    async fn pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    fn t0() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-04T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    async fn client(pool: &SqlitePool, id: &str, now: DateTime<Utc>) {
        McpOAuth::insert_client(
            pool,
            NewMcpOAuthClient {
                client_id: id,
                client_secret_hash: None,
                token_endpoint_auth_method: "none",
                client_name: Some("ChatGPT"),
                redirect_uris: &["https://chatgpt.com/cb".to_string()],
            },
            now,
        )
        .await
        .unwrap();
    }

    async fn approved(
        pool: &SqlitePool,
        id: &str,
        code: &[u8],
        now: DateTime<Utc>,
    ) -> McpOAuthAuthorization {
        McpOAuth::insert_authorization(
            pool,
            NewMcpOAuthAuthorization {
                id,
                client_id: "c1",
                redirect_uri: "https://chatgpt.com/cb",
                code_challenge: "challenge",
                scope: "mcp",
                resource: "https://vk/oauth/mcp",
                state: Some("s"),
                consent_hash: b"consent",
                expires_at: now + Duration::minutes(10),
            },
            now,
        )
        .await
        .unwrap();
        assert!(
            McpOAuth::resolve_pending(pool, id, Some((code, now + Duration::seconds(60))), now)
                .await
                .unwrap()
        );
        McpOAuth::find_authorization_by_code(pool, code)
            .await
            .unwrap()
            .unwrap()
    }

    fn pair<'a>(access: &'a [u8], refresh: &'a [u8], now: DateTime<Utc>) -> IssuedTokenHashes<'a> {
        IssuedTokenHashes {
            access_hash: access,
            access_expires_at: now + Duration::hours(1),
            refresh_hash: refresh,
            refresh_expires_at: now + Duration::days(30),
        }
    }

    #[tokio::test]
    async fn pending_consent_resolves_once() {
        let pool = pool().await;
        let now = t0();
        client(&pool, "c1", now).await;
        approved(&pool, "a1", b"code", now).await;
        // A second resolution of the same request is refused.
        assert!(
            !McpOAuth::resolve_pending(&pool, "a1", None, now)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn expired_pending_cannot_be_approved() {
        let pool = pool().await;
        let now = t0();
        client(&pool, "c1", now).await;
        McpOAuth::insert_authorization(
            &pool,
            NewMcpOAuthAuthorization {
                id: "a1",
                client_id: "c1",
                redirect_uri: "https://chatgpt.com/cb",
                code_challenge: "x",
                scope: "mcp",
                resource: "r",
                state: None,
                consent_hash: b"consent",
                expires_at: now + Duration::minutes(10),
            },
            now,
        )
        .await
        .unwrap();
        let later = now + Duration::minutes(11);
        assert!(
            !McpOAuth::resolve_pending(&pool, "a1", Some((b"code", later)), later)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn code_exchanges_once_and_tokens_verify() {
        let pool = pool().await;
        let now = t0();
        client(&pool, "c1", now).await;
        let authz = approved(&pool, "a1", b"code", now).await;
        let outcome = McpOAuth::exchange_code(&pool, &authz, "g1", pair(b"acc", b"ref", now), now)
            .await
            .unwrap();
        assert_eq!(
            outcome,
            ExchangeOutcome::Issued {
                grant_id: "g1".into()
            }
        );
        let again = McpOAuth::exchange_code(&pool, &authz, "g2", pair(b"acc2", b"ref2", now), now)
            .await
            .unwrap();
        assert_eq!(again, ExchangeOutcome::NotApproved);
        // The losing exchange left no grant behind.
        assert!(
            McpOAuth::find_token(&pool, b"acc2")
                .await
                .unwrap()
                .is_none()
        );

        let resource = "https://vk/oauth/mcp";
        assert!(
            McpOAuth::verify_access_token(&pool, b"acc", resource, now)
                .await
                .unwrap()
                .is_some()
        );
        // Refresh tokens, other resources and expired tokens never verify.
        assert!(
            McpOAuth::verify_access_token(&pool, b"ref", resource, now)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            McpOAuth::verify_access_token(&pool, b"acc", "other", now)
                .await
                .unwrap()
                .is_none()
        );
        let expired = now + Duration::hours(2);
        assert!(
            McpOAuth::verify_access_token(&pool, b"acc", resource, expired)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn expired_code_is_not_exchanged() {
        let pool = pool().await;
        let now = t0();
        client(&pool, "c1", now).await;
        let authz = approved(&pool, "a1", b"code", now).await;
        let later = now + Duration::seconds(61);
        let outcome = McpOAuth::exchange_code(&pool, &authz, "g1", pair(b"a", b"r", later), later)
            .await
            .unwrap();
        assert_eq!(outcome, ExchangeOutcome::NotApproved);
    }

    #[tokio::test]
    async fn refresh_rotates_and_reuse_is_detected() {
        let pool = pool().await;
        let now = t0();
        client(&pool, "c1", now).await;
        let authz = approved(&pool, "a1", b"code", now).await;
        McpOAuth::exchange_code(&pool, &authz, "g1", pair(b"acc", b"ref", now), now)
            .await
            .unwrap();
        let rotated =
            McpOAuth::rotate_refresh(&pool, b"ref", "g1", pair(b"acc2", b"ref2", now), now)
                .await
                .unwrap();
        assert_eq!(rotated, RotateOutcome::Rotated);
        let reuse = McpOAuth::rotate_refresh(&pool, b"ref", "g1", pair(b"acc3", b"ref3", now), now)
            .await
            .unwrap();
        assert_eq!(reuse, RotateOutcome::AlreadyRevoked);
        assert!(
            McpOAuth::find_token(&pool, b"ref")
                .await
                .unwrap()
                .unwrap()
                .revoked_at
                .is_some()
        );

        assert!(McpOAuth::revoke_grant(&pool, "g1", now).await.unwrap());
        assert!(!McpOAuth::revoke_grant(&pool, "g1", now).await.unwrap());
        let resource = "https://vk/oauth/mcp";
        assert!(
            McpOAuth::verify_access_token(&pool, b"acc2", resource, now)
                .await
                .unwrap()
                .is_none()
        );
        assert!(McpOAuth::list_live_grants(&pool).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn list_and_touch_grants() {
        let pool = pool().await;
        let now = t0();
        client(&pool, "c1", now).await;
        let authz = approved(&pool, "a1", b"code", now).await;
        McpOAuth::exchange_code(&pool, &authz, "g1", pair(b"acc", b"ref", now), now)
            .await
            .unwrap();
        McpOAuth::touch_grant(&pool, "g1", now, Duration::minutes(1))
            .await
            .unwrap();
        let grants = McpOAuth::list_live_grants(&pool).await.unwrap();
        assert_eq!(grants.len(), 1);
        assert_eq!(grants[0].client_name.as_deref(), Some("ChatGPT"));
        assert_eq!(
            grants[0].last_used_at.as_deref(),
            Some(timestamp(now).as_str())
        );
        // Within the interval, last_used_at is not rewritten.
        let soon = now + Duration::seconds(30);
        McpOAuth::touch_grant(&pool, "g1", soon, Duration::minutes(1))
            .await
            .unwrap();
        let grants = McpOAuth::list_live_grants(&pool).await.unwrap();
        assert_eq!(
            grants[0].last_used_at.as_deref(),
            Some(timestamp(now).as_str())
        );
    }

    #[tokio::test]
    async fn prune_removes_unclaimed_clients_and_expired_state() {
        let pool = pool().await;
        let now = t0();
        client(&pool, "unclaimed", now).await;
        client(&pool, "c1", now).await;
        let authz = approved(&pool, "a1", b"code", now).await;
        McpOAuth::exchange_code(&pool, &authz, "g1", pair(b"acc", b"ref", now), now)
            .await
            .unwrap();

        // A day later: the unclaimed client goes, the client with a live
        // grant stays, and the expired access token is gone while the
        // refresh token (and so the grant) survives.
        let later = now + Duration::hours(25);
        McpOAuth::prune(&pool, later).await.unwrap();
        assert!(
            McpOAuth::find_client(&pool, "unclaimed")
                .await
                .unwrap()
                .is_none()
        );
        assert!(McpOAuth::find_client(&pool, "c1").await.unwrap().is_some());
        assert!(McpOAuth::find_token(&pool, b"acc").await.unwrap().is_none());
        assert!(McpOAuth::find_token(&pool, b"ref").await.unwrap().is_some());
        assert_eq!(McpOAuth::count_clients(&pool).await.unwrap(), 1);

        // After the refresh token expires the grant and client go too.
        let much_later = now + Duration::days(32);
        McpOAuth::prune(&pool, much_later).await.unwrap();
        assert!(McpOAuth::list_live_grants(&pool).await.unwrap().is_empty());
        assert_eq!(McpOAuth::count_clients(&pool).await.unwrap(), 0);
    }
}
