//! Shared types for the SpecKit (Spec-Driven Development) viewer.
//!
//! These are the pure domain types exchanged between the local backend and the
//! web frontend. The viewer is anchored on **tasks**: the backend resolves a
//! task to its most recent live workspace and that workspace's spec-host repo
//! (see `services::services::speckit`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// The ordered stages of the SpecKit workflow, in the order they run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SpecKitStage {
    Constitution,
    Specify,
    Clarify,
    Plan,
    Tasks,
    Analyze,
    Implement,
}

impl SpecKitStage {
    /// All stages in workflow order.
    pub const ALL: [SpecKitStage; 7] = [
        SpecKitStage::Constitution,
        SpecKitStage::Specify,
        SpecKitStage::Clarify,
        SpecKitStage::Plan,
        SpecKitStage::Tasks,
        SpecKitStage::Analyze,
        SpecKitStage::Implement,
    ];
}

/// A single task parsed out of `tasks.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SpecKitTask {
    /// Task identifier as written in tasks.md (e.g. "T001"). Falls back to the
    /// 1-based ordinal when the source line has no explicit id.
    pub id: String,
    /// Human-readable task description (the text after the id/marker).
    pub description: String,
    /// File paths referenced in the task line, when present.
    pub file_paths: Vec<String>,
    /// True when the task is marked `[P]` (safe to run in parallel).
    pub parallelizable: bool,
    /// Phase / user-story heading the task is grouped under, if any.
    #[ts(optional)]
    pub phase: Option<String>,
    /// Whether the task's checkbox is ticked (`[x]`).
    pub done: bool,
}

/// A group of tasks that can run concurrently — one "column" in the dependency
/// graph. Layers are ordered; every task in layer N conceptually depends on
/// layer N-1 having completed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SpecKitTaskLayer {
    /// Task ids that make up this layer.
    pub task_ids: Vec<String>,
    /// True when the layer holds more than one task (i.e. real parallelism).
    pub parallel: bool,
}

/// Parsed `tasks.md` plus the derived parallel-execution layering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SpecKitTasks {
    pub tasks: Vec<SpecKitTask>,
    /// Ordered parallel layers derived from `[P]` markers + task order.
    pub layers: Vec<SpecKitTaskLayer>,
    pub total: u32,
    pub completed: u32,
}

/// One SpecKit artifact file. `content` is `None` when the file does not exist
/// yet on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SpecKitArtifact {
    /// Display name / filename (e.g. "spec.md").
    pub name: String,
    /// Path relative to the feature dir (e.g. "contracts/api-spec.json").
    pub relative_path: String,
    #[ts(optional)]
    pub content: Option<String>,
    pub exists: bool,
}

/// The full set of SpecKit artifacts for one feature, read off the worktree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SpecKitArtifacts {
    /// Feature dir relative to the *workspace root*, e.g.
    /// "backend/specs/vk/webhook-retries" (spec-host repo prefix included).
    pub feature_dir: String,
    pub spec: SpecKitArtifact,
    pub plan: SpecKitArtifact,
    pub tasks: SpecKitArtifact,
    pub research: SpecKitArtifact,
    pub data_model: SpecKitArtifact,
    pub quickstart: SpecKitArtifact,
    /// Contract files under `contracts/` (json / markdown), if any.
    pub contracts: Vec<SpecKitArtifact>,
}

/// Write an edited artifact back to disk.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SpecKitUpdateArtifactRequest {
    /// Path relative to the feature dir (e.g. "spec.md", "contracts/api.json").
    pub relative_path: String,
    pub content: String,
}

/// Toggle a single task's checkbox in `tasks.md`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SpecKitToggleTaskRequest {
    pub task_id: String,
    pub done: bool,
}

/// Per-stage artifact presence, surfaced as a badge in the stage rail. The
/// artifact is the stage's primary output (constitution → constitution.md,
/// specify/clarify → spec.md, plan → plan.md, tasks/analyze/implement →
/// tasks.md).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SpecKitStageArtifact {
    pub stage: SpecKitStage,
    /// The stage's primary artifact, relative to the workspace root.
    pub artifact: String,
    pub exists: bool,
}

/// Whether a workspace has SpecKit artifacts the viewer can show. Returned
/// even when there is nothing to view (`enabled: false`) so the viewer can
/// render an empty state explaining why (see `note`). The frontend picks the
/// task's most recent local workspace and asks about it by id.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SpecKitTaskStatus {
    pub workspace_id: Uuid,
    pub enabled: bool,
    /// Explains why `enabled` is false, so the frontend can show accurate
    /// copy without re-deriving it.
    #[ts(optional)]
    pub note: Option<String>,
    /// The SpecKit feature key (the workspace branch, verbatim, captured at
    /// first provisioning).
    #[ts(optional)]
    pub feature_key: Option<String>,
    /// Feature dir relative to the workspace root, e.g.
    /// "backend/specs/vk/webhook-retries".
    #[ts(optional)]
    pub feature_dir: Option<String>,
    /// The spec-host repo's dir relative to the workspace root,
    /// e.g. "backend" (repo name plus optional default working dir).
    #[ts(optional)]
    pub host_rel: Option<String>,
    /// True when the workspace has more than one repo (command files then live
    /// at the workspace root and every path is repo-qualified).
    pub multi_repo: bool,
    /// Per-stage artifact presence, in workflow order.
    pub stages: Vec<SpecKitStageArtifact>,
    /// Parsed `tasks.md`, when it exists.
    #[ts(optional)]
    pub tasks: Option<SpecKitTasks>,
}
