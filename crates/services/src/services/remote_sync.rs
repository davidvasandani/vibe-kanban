use api_types::UpsertPullRequestRequest;
use db::models::workspace::Workspace;
use git::GitService;
use sqlx::SqlitePool;
use tracing::{debug, error};
use uuid::Uuid;

use super::{
    diff_stream::{self, DiffStats},
    remote_client::{RemoteClient, RemoteClientError},
};

/// Persist accepted workspace activity and promptly propagate it to the issue store.
/// Remote failure remains best effort, as with explicit workspace edits.
pub async fn activate_workspace(
    pool: &SqlitePool,
    client: Option<&RemoteClient>,
    workspace_id: Uuid,
) -> Result<(), sqlx::Error> {
    Workspace::set_archived(pool, workspace_id, false).await?;
    if let Some(client) = client {
        // Also reconcile an already-active local workspace after a previous
        // sync failure or a remote issue-driven archive.
        let client = client.clone();
        tokio::spawn(async move {
            sync_workspace_to_remote(&client, workspace_id, None, Some(false), None).await;
        });
    }
    Ok(())
}

async fn update_workspace_on_remote(
    client: &RemoteClient,
    workspace_id: Uuid,
    name: Option<Option<String>>,
    archived: Option<bool>,
    stats: Option<&DiffStats>,
) {
    match client
        .update_workspace(
            workspace_id,
            name,
            archived,
            stats.map(|s| s.files_changed as i32),
            stats.map(|s| s.lines_added as i32),
            stats.map(|s| s.lines_removed as i32),
        )
        .await
    {
        Ok(()) => {
            debug!("Synced workspace {} to remote", workspace_id);
        }
        Err(RemoteClientError::Auth) => {
            debug!("Workspace {} sync skipped: not authenticated", workspace_id);
        }
        Err(RemoteClientError::Http { status: 404, .. }) => {
            debug!(
                "Workspace {} disappeared from remote before update, skipping sync",
                workspace_id
            );
        }
        Err(e) => {
            error!("Failed to sync workspace {} to remote: {}", workspace_id, e);
        }
    }
}

/// Syncs workspace data to the remote server.
/// First checks if the workspace exists on remote, then updates if it does.
pub async fn sync_workspace_to_remote(
    client: &RemoteClient,
    workspace_id: Uuid,
    name: Option<Option<String>>,
    archived: Option<bool>,
    stats: Option<&DiffStats>,
) {
    // First check if workspace exists on remote
    match client.workspace_exists(workspace_id).await {
        Ok(false) => {
            debug!(
                "Workspace {} not found on remote, skipping sync",
                workspace_id
            );
            return;
        }
        Err(RemoteClientError::Auth) => {
            debug!("Workspace {} sync skipped: not authenticated", workspace_id);
            return;
        }
        Err(e) => {
            error!(
                "Failed to check workspace {} existence on remote: {}",
                workspace_id, e
            );
            return;
        }
        Ok(true) => {}
    }

    // Workspace exists, proceed with update
    update_workspace_on_remote(client, workspace_id, name, archived, stats).await;
}

/// Syncs issue status to remote for a workspace merged locally without a PR.
pub async fn sync_local_workspace_merge_to_remote(client: &RemoteClient, workspace_id: Uuid) {
    match client
        .sync_issue_status_from_local_workspace_merge(workspace_id)
        .await
    {
        Ok(()) => {
            debug!(
                "Synced local workspace merge status to remote for workspace {}",
                workspace_id
            );
        }
        Err(RemoteClientError::Auth) => {
            debug!(
                "Local workspace merge sync skipped for workspace {}: not authenticated",
                workspace_id
            );
        }
        Err(RemoteClientError::Http { status: 404, .. }) => {
            debug!(
                "Local workspace merge sync skipped for workspace {}: workspace not found on remote",
                workspace_id
            );
        }
        Err(e) => {
            error!(
                "Failed to sync local workspace merge status for workspace {}: {}",
                workspace_id, e
            );
        }
    }
}

async fn upsert_pr_on_remote(client: &RemoteClient, request: UpsertPullRequestRequest) {
    let number = request.number;
    let workspace_id = request.local_workspace_id;

    // Workspace exists, proceed with PR upsert
    match client.upsert_pull_request(request).await {
        Ok(()) => {
            debug!("Synced PR #{} to remote", number);
        }
        Err(RemoteClientError::Auth) => {
            debug!("PR #{} sync skipped: not authenticated", number);
        }
        Err(RemoteClientError::Http { status: 404, .. }) => {
            debug!(
                "PR #{} workspace {} not found on remote, skipping sync",
                number, workspace_id
            );
        }
        Err(e) => {
            error!("Failed to sync PR #{} to remote: {}", number, e);
        }
    }
}

/// Syncs PR data to the remote server.
/// First checks if the workspace exists on remote, then upserts the PR if it does.
pub async fn sync_pr_to_remote(client: &RemoteClient, request: UpsertPullRequestRequest) {
    // First check if workspace exists on remote
    match client.workspace_exists(request.local_workspace_id).await {
        Ok(false) => {
            debug!(
                "PR #{} workspace {} not found on remote, skipping sync",
                request.number, request.local_workspace_id
            );
            return;
        }
        Err(RemoteClientError::Auth) => {
            debug!("PR #{} sync skipped: not authenticated", request.number);
            return;
        }
        Err(e) => {
            error!(
                "Failed to check workspace {} existence on remote: {}",
                request.local_workspace_id, e
            );
            return;
        }
        Ok(true) => {}
    }

    upsert_pr_on_remote(client, request).await;
}

/// Syncs all linked workspaces and their PRs to the remote server.
/// Used after login to catch up on any changes made while logged out.
pub async fn sync_all_linked_workspaces(
    client: &RemoteClient,
    pool: &SqlitePool,
    git: &GitService,
) {
    // Sync workspace stats
    let workspaces = match Workspace::fetch_all(pool).await {
        Ok(ws) => ws,
        Err(e) => {
            error!("Failed to fetch workspaces for post-login sync: {}", e);
            return;
        }
    };

    for workspace in &workspaces {
        match client.workspace_exists(workspace.id).await {
            Ok(true) => {}
            Ok(false) => {
                debug!(
                    "Workspace {} not found on remote, skipping post-login sync",
                    workspace.id
                );
                continue;
            }
            Err(RemoteClientError::Auth) => {
                debug!("Post-login workspace sync skipped: not authenticated");
                return;
            }
            Err(e) => {
                error!(
                    "Failed to check workspace {} existence on remote during post-login sync: {}",
                    workspace.id, e
                );
                continue;
            }
        }

        let stats = diff_stream::compute_diff_stats(pool, git, workspace).await;
        update_workspace_on_remote(
            client,
            workspace.id,
            workspace.name.clone().map(Some),
            Some(workspace.archived),
            stats.as_ref(),
        )
        .await;
    }

    debug!("Post-login workspace sync completed");
}

#[cfg(test)]
mod lifecycle_tests {
    use std::sync::Arc;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;
    use crate::services::{auth::AuthContext, oauth_credentials::OAuthCredentials};

    #[tokio::test]
    async fn activation_persists_and_sends_unarchive_even_when_locally_active() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE workspaces (id BLOB PRIMARY KEY, archived BOOLEAN, updated_at TEXT)",
        )
        .execute(&pool)
        .await
        .unwrap();
        let workspace = Uuid::new_v4();
        sqlx::query("INSERT INTO workspaces (id, archived) VALUES ($1, true)")
            .bind(workspace)
            .execute(&pool)
            .await
            .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut patches = 0;
            for _ in 0..4 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut buf = [0; 4096];
                loop {
                    let n = stream.read(&mut buf).await.unwrap();
                    assert_ne!(n, 0);
                    request.extend_from_slice(&buf[..n]);
                    if let Some(header_end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&request[..header_end]);
                        let len = headers
                            .lines()
                            .find_map(|line| {
                                let (key, value) = line.split_once(':')?;
                                key.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if request.len() >= header_end + 4 + len {
                            break;
                        }
                    }
                }
                let request = String::from_utf8(request).unwrap();
                assert!(request.starts_with("HEAD") || request.starts_with("PATCH"));
                if request.starts_with("PATCH") {
                    patches += 1;
                    let body: serde_json::Value =
                        serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
                    assert_eq!(body["local_workspace_id"], workspace.to_string());
                    assert_eq!(body["archived"], false);
                }
                stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .await
                    .unwrap();
            }
            assert_eq!(patches, 2);
        });
        let dir = tempfile::tempdir().unwrap();
        let auth = AuthContext::new(
            Arc::new(OAuthCredentials::new(dir.path().join("credentials"))),
            Default::default(),
        );
        let client = RemoteClient::new(&format!("http://{address}"), auth, true).unwrap();
        for _ in 0..2 {
            activate_workspace(&pool, Some(&client), workspace)
                .await
                .unwrap();
            let archived: bool =
                sqlx::query_scalar("SELECT archived FROM workspaces WHERE id = $1")
                    .bind(workspace)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert!(!archived);
        }
        tokio::time::timeout(std::time::Duration::from_secs(5), server)
            .await
            .unwrap()
            .unwrap();
    }
}
