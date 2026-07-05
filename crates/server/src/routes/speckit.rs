//! SpecKit (Spec-Driven Development) viewer endpoints, anchored on
//! **workspaces**. The frontend resolves a kanban issue to its most recent
//! locally-present workspace and asks about that workspace by id.
//!
//! The viewer is a read/edit layer over the artifacts the SpecKit pipeline's
//! `/speckit.*` slash commands write into the workspace's worktree, under
//! `<container_ref>/<host_rel>/specs/<feature_key>/`. The
//! spec-host repo and feature key come from
//! `services::services::speckit::resolve_speckit_host`, so provisioning, the
//! agent, and the viewer all agree on one base directory — for single- and
//! multi-repo workspaces alike.

use std::path::{Path, PathBuf};

use api_types::speckit::{
    SpecKitArtifact, SpecKitArtifacts, SpecKitStage, SpecKitStageArtifact, SpecKitTaskStatus,
    SpecKitTasks, SpecKitToggleTaskRequest, SpecKitUpdateArtifactRequest,
};
use axum::{
    Json, Router,
    extract::{Path as AxumPath, State},
    response::Json as ResponseJson,
    routing::{get, put},
};
use db::models::{session::Session, workspace::Workspace};
use deployment::Deployment;
use services::services::speckit::{self, CommandContext, SpecKitHost};
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

const NO_WORKSPACE_NOTE: &str = "Workspace not found.";
const NO_REPOS_NOTE: &str = "The task's workspace has no repositories.";
const NO_WORKTREE_NOTE: &str = "The workspace worktree is not materialized yet.";

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/speckit/workspace/{workspace_id}", get(get_status))
        .route(
            "/speckit/workspace/{workspace_id}/artifacts",
            get(get_artifacts),
        )
        .route(
            "/speckit/workspace/{workspace_id}/artifact",
            put(put_artifact),
        )
        .route(
            "/speckit/workspace/{workspace_id}/tasks/toggle",
            put(toggle_task),
        )
}

// ---------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------

/// Everything an artifact endpoint needs: the resolved spec-host and the
/// absolute feature dir (`<container_ref>/<host_rel>/specs/<feature_key>`).
struct SpecKitCtx {
    host: SpecKitHost,
    feature_abs: PathBuf,
}

async fn load_ctx(deployment: &DeploymentImpl, workspace_id: Uuid) -> Result<SpecKitCtx, ApiError> {
    let pool = &deployment.db().pool;
    let workspace = Workspace::find_by_id(pool, workspace_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest(NO_WORKSPACE_NOTE.to_string()))?;
    let host = speckit::resolve_speckit_host(pool, &workspace)
        .await?
        .ok_or_else(|| ApiError::BadRequest(NO_REPOS_NOTE.to_string()))?;
    let container_ref = workspace
        .container_ref
        .clone()
        .ok_or_else(|| ApiError::BadRequest(NO_WORKTREE_NOTE.to_string()))?;

    let workspace_root = PathBuf::from(container_ref);
    let host_abs = workspace_root.join(&host.host_rel);
    let feature_abs = host_abs.join(speckit::feature_dir(&host.feature_key));
    Ok(SpecKitCtx { host, feature_abs })
}

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

async fn get_status(
    State(deployment): State<DeploymentImpl>,
    AxumPath(workspace_id): AxumPath<Uuid>,
) -> Result<ResponseJson<ApiResponse<SpecKitTaskStatus>>, ApiError> {
    let pool = &deployment.db().pool;
    let Some(workspace) = Workspace::find_by_id(pool, workspace_id).await? else {
        return Ok(ResponseJson(ApiResponse::success(disabled_status(
            workspace_id,
            NO_WORKSPACE_NOTE,
        ))));
    };
    let Some(host) = speckit::resolve_speckit_host(pool, &workspace).await? else {
        return Ok(ResponseJson(ApiResponse::success(disabled_status(
            workspace_id,
            NO_REPOS_NOTE,
        ))));
    };
    let Some(container_ref) = workspace.container_ref.clone() else {
        return Ok(ResponseJson(ApiResponse::success(disabled_status(
            workspace_id,
            NO_WORKTREE_NOTE,
        ))));
    };

    let workspace_root = PathBuf::from(container_ref);
    let host_abs = workspace_root.join(&host.host_rel);
    let feature_abs = host_abs.join(speckit::feature_dir(&host.feature_key));

    // Best-effort defensive repair, but ONLY for workspaces that were actually
    // provisioned as SpecKit (durable gate): viewing a non-SpecKit task must
    // never sprinkle scaffold files into its worktree.
    if workspace.speckit_feature_key.is_some() {
        let rel = Session::resolve_agent_working_dir(pool, workspace.id).await?;
        let agent_cwd = workspace_root.join(rel.as_deref().unwrap_or(""));
        if let Err(e) =
            speckit::ensure_scaffold(&host_abs, &agent_cwd, &CommandContext::from(&host))
        {
            tracing::warn!(?e, %workspace_id, "Failed to repair SpecKit scaffold");
        }
    }

    let feature_dir_ws = format!(
        "{}/{}",
        host.host_rel,
        speckit::feature_dir(&host.feature_key)
    );
    let stages = stage_artifacts(&host, &host_abs, &feature_abs, &feature_dir_ws);
    let tasks = std::fs::read_to_string(feature_abs.join("tasks.md"))
        .ok()
        .map(|text| speckit::parse_tasks_md(&text));

    Ok(ResponseJson(ApiResponse::success(SpecKitTaskStatus {
        workspace_id,
        enabled: true,
        note: None,
        feature_key: Some(host.feature_key.clone()),
        feature_dir: Some(feature_dir_ws),
        host_rel: Some(host.host_rel.clone()),
        multi_repo: host.multi_repo,
        stages,
        tasks,
    })))
}

fn disabled_status(workspace_id: Uuid, note: &str) -> SpecKitTaskStatus {
    SpecKitTaskStatus {
        workspace_id,
        enabled: false,
        note: Some(note.to_string()),
        feature_key: None,
        feature_dir: None,
        host_rel: None,
        multi_repo: false,
        stages: Vec::new(),
        tasks: None,
    }
}

/// Per-stage artifact presence: each stage's primary output, with paths
/// reported relative to the workspace root.
fn stage_artifacts(
    host: &SpecKitHost,
    host_abs: &Path,
    feature_abs: &Path,
    feature_dir_ws: &str,
) -> Vec<SpecKitStageArtifact> {
    let constitution_ws = format!("{}/{}", host.host_rel, speckit::CONSTITUTION_REL_PATH);
    SpecKitStage::ALL
        .iter()
        .map(|&stage| {
            let (artifact, abs) = match stage {
                SpecKitStage::Constitution => (
                    constitution_ws.clone(),
                    host_abs.join(speckit::CONSTITUTION_REL_PATH),
                ),
                SpecKitStage::Specify | SpecKitStage::Clarify => (
                    format!("{feature_dir_ws}/spec.md"),
                    feature_abs.join("spec.md"),
                ),
                SpecKitStage::Plan => (
                    format!("{feature_dir_ws}/plan.md"),
                    feature_abs.join("plan.md"),
                ),
                SpecKitStage::Tasks | SpecKitStage::Analyze | SpecKitStage::Implement => (
                    format!("{feature_dir_ws}/tasks.md"),
                    feature_abs.join("tasks.md"),
                ),
            };
            SpecKitStageArtifact {
                stage,
                artifact,
                exists: abs.is_file(),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Artifacts
// ---------------------------------------------------------------------------

async fn get_artifacts(
    State(deployment): State<DeploymentImpl>,
    AxumPath(workspace_id): AxumPath<Uuid>,
) -> Result<ResponseJson<ApiResponse<SpecKitArtifacts>>, ApiError> {
    let ctx = load_ctx(&deployment, workspace_id).await?;

    let artifacts = SpecKitArtifacts {
        feature_dir: format!(
            "{}/{}",
            ctx.host.host_rel,
            speckit::feature_dir(&ctx.host.feature_key)
        ),
        spec: read_artifact(&ctx.feature_abs, "spec.md"),
        plan: read_artifact(&ctx.feature_abs, "plan.md"),
        tasks: read_artifact(&ctx.feature_abs, "tasks.md"),
        research: read_artifact(&ctx.feature_abs, "research.md"),
        data_model: read_artifact(&ctx.feature_abs, "data-model.md"),
        quickstart: read_artifact(&ctx.feature_abs, "quickstart.md"),
        contracts: read_contracts(&ctx.feature_abs),
    };
    Ok(ResponseJson(ApiResponse::success(artifacts)))
}

async fn put_artifact(
    State(deployment): State<DeploymentImpl>,
    AxumPath(workspace_id): AxumPath<Uuid>,
    Json(payload): Json<SpecKitUpdateArtifactRequest>,
) -> Result<ResponseJson<ApiResponse<SpecKitArtifact>>, ApiError> {
    let ctx = load_ctx(&deployment, workspace_id).await?;
    let target = safe_join(&ctx.feature_abs, &payload.relative_path)?;
    std::fs::write(&target, &payload.content)?;

    Ok(ResponseJson(ApiResponse::success(SpecKitArtifact {
        name: file_name_of(&payload.relative_path),
        relative_path: payload.relative_path,
        content: Some(payload.content),
        exists: true,
    })))
}

// ---------------------------------------------------------------------------
// Tasks
// ---------------------------------------------------------------------------

async fn toggle_task(
    State(deployment): State<DeploymentImpl>,
    AxumPath(workspace_id): AxumPath<Uuid>,
    Json(payload): Json<SpecKitToggleTaskRequest>,
) -> Result<ResponseJson<ApiResponse<SpecKitTasks>>, ApiError> {
    let ctx = load_ctx(&deployment, workspace_id).await?;
    let tasks_path = ctx.feature_abs.join("tasks.md");
    let text = std::fs::read_to_string(&tasks_path)
        .map_err(|_| ApiError::BadRequest("tasks.md does not exist yet.".to_string()))?;
    let updated = speckit::toggle_task(&text, &payload.task_id, payload.done);
    std::fs::write(&tasks_path, &updated)?;
    Ok(ResponseJson(ApiResponse::success(speckit::parse_tasks_md(
        &updated,
    ))))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn read_artifact(feature_abs: &Path, name: &str) -> SpecKitArtifact {
    let path = feature_abs.join(name);
    let content = std::fs::read_to_string(&path).ok();
    SpecKitArtifact {
        name: name.to_string(),
        relative_path: name.to_string(),
        exists: content.is_some(),
        content,
    }
}

fn read_contracts(feature_abs: &Path) -> Vec<SpecKitArtifact> {
    let dir = feature_abs.join("contracts");
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return out;
    };
    let mut files: Vec<_> = entries.flatten().filter(|e| e.path().is_file()).collect();
    files.sort_by_key(|e| e.file_name());
    for entry in files {
        let name = entry.file_name().to_string_lossy().to_string();
        let content = std::fs::read_to_string(entry.path()).ok();
        out.push(SpecKitArtifact {
            relative_path: format!("contracts/{name}"),
            exists: content.is_some(),
            content,
            name,
        });
    }
    out
}

/// Join a caller-supplied relative path to the feature dir, rejecting
/// traversal both lexically (no absolute paths, no `..` components) and
/// physically: the write target's parent is canonicalized (after creation)
/// and must resolve *under* the canonicalized feature dir, so symlinks inside
/// the worktree cannot redirect the write outside it.
#[allow(clippy::result_large_err)]
fn safe_join(base: &Path, rel: &str) -> Result<PathBuf, ApiError> {
    let rel = rel.trim();
    if rel.is_empty() {
        return Err(ApiError::BadRequest("Empty path.".to_string()));
    }
    let candidate = Path::new(rel);
    if candidate.is_absolute()
        || candidate
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(ApiError::BadRequest("Invalid artifact path.".to_string()));
    }

    let joined = base.join(candidate);
    let file_name = joined
        .file_name()
        .map(|n| n.to_os_string())
        .ok_or_else(|| ApiError::BadRequest("Invalid artifact path.".to_string()))?;
    let parent = joined
        .parent()
        .ok_or_else(|| ApiError::BadRequest("Invalid artifact path.".to_string()))?
        .to_path_buf();

    std::fs::create_dir_all(base)?;
    std::fs::create_dir_all(&parent)?;
    let canonical_base = base.canonicalize()?;
    let canonical_parent = parent.canonicalize()?;
    if !canonical_parent.starts_with(&canonical_base) {
        return Err(ApiError::BadRequest("Invalid artifact path.".to_string()));
    }
    Ok(canonical_parent.join(file_name))
}

fn file_name_of(rel: &str) -> String {
    Path::new(rel)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| rel.to_string())
}
