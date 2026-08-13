//! SpecKit (Spec-Driven Development) service logic.
//!
//! This module owns the *pure* pieces of the SpecKit workflow plus its
//! provisioning:
//! - mapping a [`SpecKitStage`] to its slash command
//! - resolving a workspace's **spec-host repo** (multi-repo aware)
//! - generating fully-literal `/speckit.*` command files (no "current git
//!   branch" derivation anywhere — the backend bakes exact paths in)
//! - parsing `tasks.md` into structured tasks + parallel-execution layers
//! - toggling a task's checkbox in `tasks.md`
//! - provisioning the `.specify/` scaffold + command files into a worktree
//!
//! # Multi-repo design
//!
//! - The **feature key** is the workspace's `branch`, verbatim (nested dirs
//!   like `vk/foo` are fine). It is persisted on the workspace at first
//!   provisioning (`workspaces.speckit_feature_key`) and thereafter always
//!   reused — never re-derived by the agent or from git.
//! - `specs/` and `.specify/` live in exactly one repo worktree: the
//!   **spec-host repo** (`workspaces.speckit_host_repo_id`). Resolution order:
//!   persisted host (if that repo is still in the workspace) → the only repo
//!   (single-repo) → first repo by `display_name` ASC, `id` ASC.
//! - Command files are written at the **agent's effective cwd**: the repo root
//!   for single-repo workspaces (paths in bodies are repo-relative), the
//!   workspace root for multi-repo ones (every path is repo-qualified with the
//!   host repo's dir).
//!
//! The pipeline's execution agent is the single driver of SpecKit stages (via
//! `/speckit.*` slash commands); the route layer (`server::routes::speckit`)
//! is a read/edit **viewer** over the pipeline's artifacts on disk.

use std::{
    io,
    path::{Path, PathBuf},
};

use api_types::speckit::{SpecKitStage, SpecKitTask, SpecKitTaskLayer, SpecKitTasks};
use db::models::{repo::Repo, session::Session, workspace::Workspace};
use sqlx::SqlitePool;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum SpecKitError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
}

// ---------------------------------------------------------------------------
// Stage metadata
// ---------------------------------------------------------------------------

/// The Claude Code slash command that drives a stage.
pub fn slash_command(stage: SpecKitStage) -> &'static str {
    match stage {
        SpecKitStage::Constitution => "/speckit.constitution",
        SpecKitStage::Specify => "/speckit.specify",
        SpecKitStage::Clarify => "/speckit.clarify",
        SpecKitStage::Plan => "/speckit.plan",
        SpecKitStage::Tasks => "/speckit.tasks",
        SpecKitStage::Analyze => "/speckit.analyze",
        SpecKitStage::Implement => "/speckit.implement",
    }
}

// ---------------------------------------------------------------------------
// Pipeline gating
// ---------------------------------------------------------------------------

/// Whether a coding-agent prompt opts into the SpecKit pipeline.
///
/// Tightened gate: requires **both** a composed `## Pipeline` block *and* a
/// `/speckit.` slash-command mention, so a prose mention of `/speckit.` in an
/// ordinary task description does not trigger provisioning.
pub fn is_speckit_pipeline(prompt: &str) -> bool {
    prompt.contains("## Pipeline") && prompt.contains("/speckit.")
}

// ---------------------------------------------------------------------------
// Feature dir
// ---------------------------------------------------------------------------

/// The feature dir relative to the spec-host repo root, e.g.
/// `specs/vk/webhook-retries`. The argument is the persisted feature key
/// (the workspace branch, verbatim).
pub fn feature_dir(feature_key: &str) -> String {
    format!("specs/{feature_key}")
}

// ---------------------------------------------------------------------------
// Spec-host resolution
// ---------------------------------------------------------------------------

/// The resolved SpecKit anchor for a workspace: which repo hosts `specs/` +
/// `.specify/`, where that repo lives relative to the workspace root, and the
/// feature key the artifacts are filed under.
#[derive(Debug, Clone)]
pub struct SpecKitHost {
    /// The spec-host repo (owns `specs/` + `.specify/`).
    pub repo: Repo,
    /// The host repo's dir relative to the workspace root:
    /// `<repo.name>[/<default_working_dir>]`.
    pub host_rel: String,
    /// The feature key (persisted `speckit_feature_key`, else the workspace
    /// branch verbatim).
    pub feature_key: String,
    /// True when the workspace has more than one repo.
    pub multi_repo: bool,
}

/// The host repo's dir relative to the workspace root.
fn repo_host_rel(repo: &Repo) -> String {
    match repo.default_working_dir.as_deref() {
        Some(subdir) if !subdir.is_empty() => PathBuf::from(&repo.name)
            .join(subdir)
            .to_string_lossy()
            .to_string(),
        _ => repo.name.clone(),
    }
}

/// Pick the spec-host repo out of a workspace's repos:
/// persisted host (if still present) → the only repo → first by
/// `display_name` ASC, `id` ASC. `None` only when there are no repos.
pub fn select_host_repo(persisted: Option<Uuid>, repos: &[Repo]) -> Option<&Repo> {
    if let Some(id) = persisted
        && let Some(repo) = repos.iter().find(|r| r.id == id)
    {
        return Some(repo);
    }
    repos
        .iter()
        .min_by(|a, b| a.display_name.cmp(&b.display_name).then(a.id.cmp(&b.id)))
}

/// Pure core of [`resolve_speckit_host`]: assemble the host from the
/// workspace's persisted SpecKit columns, its branch, and its repos.
pub fn build_host(
    persisted_key: Option<&str>,
    branch: &str,
    persisted_repo: Option<Uuid>,
    repos: &[Repo],
) -> Option<SpecKitHost> {
    let repo = select_host_repo(persisted_repo, repos)?.clone();
    let host_rel = repo_host_rel(&repo);
    let feature_key = persisted_key.unwrap_or(branch).to_string();
    Some(SpecKitHost {
        host_rel,
        feature_key,
        multi_repo: repos.len() > 1,
        repo,
    })
}

/// Resolve the workspace's spec-host repo + feature key. `Ok(None)` when the
/// workspace has no repos (nothing to anchor on).
pub async fn resolve_speckit_host(
    pool: &SqlitePool,
    workspace: &Workspace,
) -> Result<Option<SpecKitHost>, sqlx::Error> {
    let repos =
        db::models::workspace_repo::WorkspaceRepo::find_repos_for_workspace(pool, workspace.id)
            .await?;
    Ok(build_host(
        workspace.speckit_feature_key.as_deref(),
        &workspace.branch,
        workspace.speckit_host_repo_id,
        &repos,
    ))
}

// ---------------------------------------------------------------------------
// tasks.md parsing
// ---------------------------------------------------------------------------

/// One scanned task line, retaining enough to rewrite the line in place.
struct ScannedTask {
    line_index: usize,
    /// Byte index, within the line, of the character inside the `[ ]` checkbox.
    checkbox_char_index: Option<usize>,
    id: String,
    description: String,
    file_paths: Vec<String>,
    parallelizable: bool,
    phase: Option<String>,
    done: bool,
}

/// Parse `tasks.md` into structured tasks plus the derived parallel layers.
pub fn parse_tasks_md(text: &str) -> SpecKitTasks {
    let scanned = scan_tasks(text);
    let tasks: Vec<SpecKitTask> = scanned
        .into_iter()
        .map(|s| SpecKitTask {
            id: s.id,
            description: s.description,
            file_paths: s.file_paths,
            parallelizable: s.parallelizable,
            phase: s.phase,
            done: s.done,
        })
        .collect();
    let completed = tasks.iter().filter(|t| t.done).count() as u32;
    let layers = compute_layers(&tasks);
    SpecKitTasks {
        total: tasks.len() as u32,
        completed,
        layers,
        tasks,
    }
}

/// Group tasks into ordered parallel-execution layers.
///
/// A run of consecutive `[P]` tasks forms one layer (they can run together).
/// A non-`[P]` task is a barrier: it forms its own singleton layer that runs
/// after the preceding group. `parallel` is true only when a layer holds more
/// than one task.
pub fn compute_layers(tasks: &[SpecKitTask]) -> Vec<SpecKitTaskLayer> {
    let mut layers: Vec<SpecKitTaskLayer> = Vec::new();
    let mut current: Vec<String> = Vec::new();

    let flush = |current: &mut Vec<String>, layers: &mut Vec<SpecKitTaskLayer>| {
        if !current.is_empty() {
            let task_ids = std::mem::take(current);
            layers.push(SpecKitTaskLayer {
                parallel: task_ids.len() > 1,
                task_ids,
            });
        }
    };

    for task in tasks {
        if task.parallelizable {
            current.push(task.id.clone());
        } else {
            flush(&mut current, &mut layers);
            layers.push(SpecKitTaskLayer {
                task_ids: vec![task.id.clone()],
                parallel: false,
            });
        }
    }
    flush(&mut current, &mut layers);
    layers
}

/// Toggle a task's checkbox by id, returning the rewritten `tasks.md`.
///
/// Matches the same ids `parse_tasks_md` reports (including fallback ordinals),
/// so the frontend round-trips cleanly. Returns the text unchanged if the id
/// isn't found or its line has no checkbox.
pub fn toggle_task(text: &str, task_id: &str, done: bool) -> String {
    let scanned = scan_tasks(text);
    let Some(target) = scanned.iter().find(|s| s.id == task_id) else {
        return text.to_string();
    };
    let Some(char_idx) = target.checkbox_char_index else {
        return text.to_string();
    };

    let mut lines: Vec<&str> = text.split('\n').collect();
    let Some(line) = lines.get(target.line_index).copied() else {
        return text.to_string();
    };
    let new_char = if done { 'x' } else { ' ' };
    let mut rewritten = String::with_capacity(line.len());
    rewritten.push_str(&line[..char_idx]);
    rewritten.push(new_char);
    rewritten.push_str(&line[char_idx + 1..]);
    lines[target.line_index] = &rewritten;
    lines.join("\n")
}

/// Scan `tasks.md` line by line, tracking the current phase heading and HTML
/// comment regions, and extract task lines with byte offsets for rewriting.
fn scan_tasks(text: &str) -> Vec<ScannedTask> {
    let mut out = Vec::new();
    let mut current_phase: Option<String> = None;
    let mut in_comment = false;
    let mut ordinal = 0u32;

    for (line_index, raw) in text.split('\n').enumerate() {
        let line = raw;
        let trimmed = line.trim();

        // Skip HTML comment regions so the template's conventions block doesn't
        // get parsed as tasks.
        if in_comment {
            if trimmed.contains("-->") {
                in_comment = false;
            }
            continue;
        }
        if trimmed.starts_with("<!--") {
            if !trimmed.contains("-->") {
                in_comment = true;
            }
            continue;
        }

        // Headings (level >= 2) set the current phase.
        if let Some(rest) = trimmed.strip_prefix("##") {
            let heading = rest.trim_start_matches('#').trim();
            if !heading.is_empty() {
                current_phase = Some(heading.to_string());
            }
            continue;
        }

        if let Some(mut parsed) = parse_task_line(line) {
            ordinal += 1;
            if parsed.id.is_empty() {
                parsed.id = format!("T{ordinal:03}");
            }
            out.push(ScannedTask {
                line_index,
                checkbox_char_index: parsed.checkbox_char_index,
                id: parsed.id,
                description: parsed.description,
                file_paths: parsed.file_paths,
                parallelizable: parsed.parallelizable,
                phase: current_phase.clone(),
                done: parsed.done,
            });
        }
    }
    out
}

struct ParsedLine {
    checkbox_char_index: Option<usize>,
    id: String,
    description: String,
    file_paths: Vec<String>,
    parallelizable: bool,
    done: bool,
}

/// Parse a single line into a task, or `None` if it isn't a task bullet.
///
/// Accepts `- [ ] T001 [P] Description`, `* [x] T002 ...`, and id-only bullets
/// `- T003 ...`. A line qualifies as a task only if it has a checkbox or a
/// `T<number>` id.
fn parse_task_line(line: &str) -> Option<ParsedLine> {
    let lead_ws = line.len() - line.trim_start().len();
    let after_ws = &line[lead_ws..];

    // Require a list bullet.
    let after_bullet = after_ws
        .strip_prefix("- ")
        .or_else(|| after_ws.strip_prefix("* "))
        .or_else(|| after_ws.strip_prefix("-\t"))
        .or_else(|| after_ws.strip_prefix("*\t"))?;
    let bullet_offset = line.len() - after_bullet.len();

    // Optional checkbox `[ ]` / `[x]` / `[X]`.
    let mut done = false;
    let mut checkbox_char_index = None;
    let mut rest = after_bullet;
    let cb_lead = after_bullet.len() - after_bullet.trim_start().len();
    let cb_candidate = &after_bullet[cb_lead..];
    let bytes = cb_candidate.as_bytes();
    if bytes.first() == Some(&b'[') && bytes.get(2) == Some(&b']') {
        let c = bytes[1];
        if c == b' ' || c == b'x' || c == b'X' {
            done = c == b'x' || c == b'X';
            checkbox_char_index = Some(bullet_offset + cb_lead + 1);
            rest = &cb_candidate[3..];
        }
    }

    // Strip a leading bold marker and whitespace.
    let mut rest = rest.trim_start();
    if let Some(r) = rest.strip_prefix("**") {
        rest = r.trim_start();
    }

    // Optional `T<number>` id as the first token.
    let mut id = String::new();
    if let Some(first) = rest.split_whitespace().next()
        && let Some(parsed_id) = extract_id(first)
    {
        id = parsed_id;
        // Advance past the id token.
        if let Some(pos) = rest.find(first) {
            rest = rest[pos + first.len()..].trim_start();
        }
    }

    // Not a task line unless we found a checkbox or an id.
    if checkbox_char_index.is_none() && id.is_empty() {
        return None;
    }

    // Optional `[P]` parallel marker.
    let mut parallelizable = false;
    if let Some(r) = rest
        .strip_prefix("[P]")
        .or_else(|| rest.strip_prefix("[p]"))
    {
        parallelizable = true;
        rest = r.trim_start();
    }

    let description = rest
        .trim()
        .trim_end_matches("**")
        .trim_start_matches("**")
        .trim()
        .to_string();
    let file_paths = extract_file_paths(&description);

    Some(ParsedLine {
        checkbox_char_index,
        id,
        description,
        file_paths,
        parallelizable,
        done,
    })
}

/// Recognize a `T<number>` id token, tolerating trailing punctuation / bold.
fn extract_id(token: &str) -> Option<String> {
    let t = token.trim_matches('*').trim_end_matches([':', '.', ')']);
    let mut chars = t.chars();
    let first = chars.next()?;
    if first != 'T' && first != 't' {
        return None;
    }
    let digits: String = chars.collect();
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!("T{digits}"))
}

/// Pull file paths out of a task description: backtick-wrapped spans first, then
/// any bare slash-containing tokens.
fn extract_file_paths(description: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut rest = description;
    while let Some(start) = rest.find('`') {
        let after = &rest[start + 1..];
        if let Some(end) = after.find('`') {
            let span = after[..end].trim();
            if span.contains('/') || span.contains('.') {
                paths.push(span.to_string());
            }
            rest = &after[end + 1..];
        } else {
            break;
        }
    }
    if paths.is_empty() {
        for tok in description.split_whitespace() {
            let cleaned = tok.trim_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != '.');
            if cleaned.contains('/') && !cleaned.is_empty() {
                paths.push(cleaned.to_string());
            }
        }
    }
    paths
}

// ---------------------------------------------------------------------------
// Scaffold provisioning
// ---------------------------------------------------------------------------

const CONSTITUTION_TEMPLATE: &str =
    include_str!("../../../../../assets/speckit/memory/constitution.md");
const SPEC_TEMPLATE: &str =
    include_str!("../../../../../assets/speckit/templates/spec-template.md");
const PLAN_TEMPLATE: &str =
    include_str!("../../../../../assets/speckit/templates/plan-template.md");
const TASKS_TEMPLATE: &str =
    include_str!("../../../../../assets/speckit/templates/tasks-template.md");

/// Where the constitution lives relative to the spec-host repo root.
pub const CONSTITUTION_REL_PATH: &str = ".specify/memory/constitution.md";
const FEATURE_OWNER_FILE: &str = ".speckit-owner";

/// Everything [`command_file`] needs to bake fully-literal paths into a
/// `/speckit.*` command body: paths are written relative to the **agent's
/// effective cwd** (the host repo root for single-repo workspaces, the
/// workspace root — with every path prefixed by `host_rel` — for multi-repo
/// ones).
#[derive(Debug, Clone)]
pub struct CommandContext {
    pub host_rel: String,
    pub feature_key: String,
    pub multi_repo: bool,
}

impl From<&SpecKitHost> for CommandContext {
    fn from(host: &SpecKitHost) -> Self {
        CommandContext {
            host_rel: host.host_rel.clone(),
            feature_key: host.feature_key.clone(),
            multi_repo: host.multi_repo,
        }
    }
}

impl CommandContext {
    /// The path prefix that repo-qualifies every artifact path when the agent
    /// runs at the workspace root (multi-repo). Empty for single-repo.
    fn prefix(&self) -> String {
        if self.multi_repo {
            format!("{}/", self.host_rel)
        } else {
            String::new()
        }
    }
}

/// The host-side scaffold (constitution + templates) as
/// (path relative to the host repo root, content) pairs. These are
/// **skip-if-exists**: operator edits (especially the constitution) survive
/// re-runs.
fn host_scaffold_files() -> Vec<(PathBuf, String)> {
    vec![
        (
            PathBuf::from(CONSTITUTION_REL_PATH),
            CONSTITUTION_TEMPLATE.to_string(),
        ),
        (
            PathBuf::from(".specify/templates/spec-template.md"),
            SPEC_TEMPLATE.to_string(),
        ),
        (
            PathBuf::from(".specify/templates/plan-template.md"),
            PLAN_TEMPLATE.to_string(),
        ),
        (
            PathBuf::from(".specify/templates/tasks-template.md"),
            TASKS_TEMPLATE.to_string(),
        ),
    ]
}

/// The command files as (path relative to the agent cwd, content) pairs.
/// These are **overwritten on content mismatch**: they are generated (paths
/// baked in), so a stale copy from an earlier branch/host must not survive.
fn command_scaffold_files(ctx: &CommandContext) -> Vec<(PathBuf, String)> {
    SpecKitStage::ALL
        .iter()
        .map(|&stage| {
            let name = slash_command(stage).trim_start_matches('/');
            (
                PathBuf::from(format!(".claude/commands/{name}.md")),
                command_file(stage, ctx),
            )
        })
        .collect()
}

/// Generate a Claude Code slash-command file for a stage. Every artifact path
/// is a **literal** (repo-qualified in multi-repo workspaces) — nothing is
/// derived from the current git branch by the agent.
pub fn command_file(stage: SpecKitStage, ctx: &CommandContext) -> String {
    let p = ctx.prefix();
    let key = &ctx.feature_key;
    let body = match stage {
        SpecKitStage::Constitution => format!(
            "Create or update the project constitution at `{p}.specify/memory/constitution.md`. \
             Use the templates under `{p}.specify/templates/` for structure if helpful. Capture \
             the principles in $ARGUMENTS (or refine the existing ones). Keep it concise and \
             enforceable."
        ),
        SpecKitStage::Specify => format!(
            "Write the feature specification. Read `{p}.specify/templates/spec-template.md` and \
             `{p}.specify/memory/constitution.md`. Write the spec to `{p}specs/{key}/spec.md`. \
             Focus on WHAT and WHY (functional requirements, user stories, acceptance criteria) \
             — not the tech stack. Mark anything unclear with `[NEEDS CLARIFICATION: ...]`. The \
             feature description: $ARGUMENTS"
        ),
        SpecKitStage::Clarify => format!(
            "Review `{p}specs/{key}/spec.md` and resolve underspecified areas. If answers are \
             provided in $ARGUMENTS, fold them in and remove the matching `[NEEDS CLARIFICATION]` \
             markers. List any questions that remain open."
        ),
        SpecKitStage::Plan => format!(
            "Read `{p}specs/{key}/spec.md`, the constitution at \
             `{p}.specify/memory/constitution.md`, and `{p}.specify/templates/plan-template.md`. \
             Write the technical plan to `{p}specs/{key}/plan.md`, and, when relevant, \
             `research.md`, `data-model.md`, and `contracts/` in that same directory. Ground \
             every step in real files. Confirm the approach honors the constitution."
        ),
        SpecKitStage::Tasks => format!(
            "Read `{p}specs/{key}/plan.md` and `{p}.specify/templates/tasks-template.md`. Write \
             `{p}specs/{key}/tasks.md`: dependency-ordered tasks with stable `T###` ids, `[P]` on \
             tasks that touch independent files (parallel-safe), and the exact file path(s) each \
             task changes."
        ),
        SpecKitStage::Analyze => format!(
            "Cross-check `spec.md`, `plan.md`, and `tasks.md` under `{p}specs/{key}/` against \
             the constitution at `{p}.specify/memory/constitution.md` for inconsistencies, \
             coverage gaps, and constitution violations. Report findings as a list, each tagged \
             error/warning/info and naming the artifact it concerns. Do not modify files."
        ),
        SpecKitStage::Implement => format!(
            "Execute `{p}specs/{key}/tasks.md` in dependency order. Tasks marked `[P]` within \
             the same group may be done together. As you finish each task, mark its checkbox \
             `[x]` in `{p}specs/{key}/tasks.md`. Follow the plan and the constitution at \
             `{p}.specify/memory/constitution.md`."
        ),
    };
    format!("# {cmd}\n\n{body}\n", cmd = slash_command(stage))
}

fn declared_spec_owner(spec: &str) -> Option<String> {
    spec.lines().find_map(|line| {
        let value = line.strip_prefix("**Task id**:")?.trim();
        Some(value.trim_matches('`').to_string())
    })
}

/// Claim the feature directory before generating commands that instruct an
/// agent to write into it. A persisted marker is authoritative; for directories
/// created before markers existed, the spec supplies migration evidence.
fn ensure_feature_dir_owner(host_root: &Path, ctx: &CommandContext) -> io::Result<()> {
    let dir = host_root.join(feature_dir(&ctx.feature_key));
    let owner_path = dir.join(FEATURE_OWNER_FILE);
    let expected = &ctx.feature_key;

    if let Ok(owner) = std::fs::read_to_string(&owner_path) {
        if owner.trim() != expected {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "SpecKit directory {} belongs to task {}, not {}",
                    dir.display(),
                    owner.trim(),
                    expected
                ),
            ));
        }
        return Ok(());
    }

    let spec_path = dir.join("spec.md");
    if let Ok(spec) = std::fs::read_to_string(&spec_path) {
        let declared_owner = declared_spec_owner(&spec);
        let expected_dir = format!("**Feature dir**: `specs/{expected}/`");
        let matches_legacy_dir = spec.lines().any(|line| line.trim() == expected_dir);
        if declared_owner.as_deref().is_some_and(|owner| owner != expected)
            || (declared_owner.is_none() && !matches_legacy_dir)
        {
            let actual = declared_owner.unwrap_or_else(|| "unknown legacy owner".to_string());
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "SpecKit directory {} belongs to task {}, not {}",
                    dir.display(),
                    actual,
                    expected
                ),
            ));
        }
    }

    write_with_parents(&owner_path, &format!("{expected}\n"))
}

/// Ensure the `.specify/` scaffold (under the spec-host repo root) and the
/// `/speckit.*` command files (under the agent's effective cwd) exist.
///
/// - Constitution + templates are **skip-if-exists** (operator edits survive).
/// - Command files are generated and **overwritten on content mismatch** so
///   baked-in paths never go stale.
pub fn ensure_scaffold(host_root: &Path, agent_cwd: &Path, ctx: &CommandContext) -> io::Result<()> {
    ensure_feature_dir_owner(host_root, ctx)?;
    for (rel, content) in host_scaffold_files() {
        let path = host_root.join(&rel);
        if path.exists() {
            continue;
        }
        write_with_parents(&path, &content)?;
    }
    for (rel, content) in command_scaffold_files(ctx) {
        let path = agent_cwd.join(&rel);
        if std::fs::read_to_string(&path).is_ok_and(|existing| existing == content) {
            continue;
        }
        write_with_parents(&path, &content)?;
    }
    Ok(())
}

fn write_with_parents(path: &Path, content: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)
}

// ---------------------------------------------------------------------------
// Provisioning
// ---------------------------------------------------------------------------

/// Provision SpecKit for a workspace: resolve the spec-host, persist the host
/// repo + feature key (first time only — the guarded UPDATE is a no-op once a
/// key exists), and write the scaffold + command files into the worktree.
///
/// Quietly does nothing when the workspace has no repos or no materialized
/// worktree yet.
pub async fn provision_workspace(
    pool: &SqlitePool,
    workspace: &Workspace,
) -> Result<(), SpecKitError> {
    let Some(host) = resolve_speckit_host(pool, workspace).await? else {
        return Ok(());
    };
    let Some(container_ref) = workspace.container_ref.as_deref() else {
        return Ok(());
    };

    // Persist host + feature key at FIRST provisioning only (SQL-guarded).
    Workspace::set_speckit_provisioning(pool, workspace.id, host.repo.id, &host.feature_key)
        .await?;

    // Re-resolve from the freshly persisted row so a concurrent first
    // provisioning that won the guarded write (persisting a different host/key)
    // is honored — the scaffold must anchor on the authoritative persisted
    // values, not the possibly-stale ones we proposed above.
    let host = match Workspace::find_by_id(pool, workspace.id).await? {
        Some(reloaded) => resolve_speckit_host(pool, &reloaded).await?.unwrap_or(host),
        None => host,
    };

    let workspace_root = Path::new(container_ref);
    let rel = Session::resolve_agent_working_dir(pool, workspace.id).await?;
    let agent_cwd = workspace_root.join(rel.as_deref().unwrap_or(""));
    let host_root = workspace_root.join(&host.host_rel);
    ensure_scaffold(&host_root, &agent_cwd, &CommandContext::from(&host))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    const SAMPLE: &str = r#"# Tasks: Webhook retries

## Phase 1: Setup
- [ ] T001 [P] Create module in `src/retry/mod.rs`
- [x] T002 [P] Add types in `src/retry/types.rs`

## Phase 2: Core
- [ ] T003 Implement core in `src/retry/core.rs`
- [ ] T004 [P] Tests in `src/retry/tests.rs`

<!--
- [ ] T999 this is inside a comment and must be ignored
-->
"#;

    fn mk_repo(id: Uuid, name: &str, display_name: &str, working_dir: Option<&str>) -> Repo {
        Repo {
            id,
            path: PathBuf::from(format!("/repos/{name}")),
            name: name.to_string(),
            display_name: display_name.to_string(),
            setup_script: None,
            cleanup_script: None,
            archive_script: None,
            copy_files: None,
            parallel_setup_script: false,
            dev_server_script: None,
            default_target_branch: None,
            default_working_dir: working_dir.map(str::to_string),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn uuid(n: u8) -> Uuid {
        Uuid::from_bytes([n; 16])
    }

    // -- tasks.md engine ----------------------------------------------------

    #[test]
    fn parses_tasks_with_ids_phases_and_markers() {
        let parsed = parse_tasks_md(SAMPLE);
        assert_eq!(parsed.total, 4);
        assert_eq!(parsed.completed, 1);

        let ids: Vec<&str> = parsed.tasks.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["T001", "T002", "T003", "T004"]);

        let t1 = &parsed.tasks[0];
        assert!(t1.parallelizable);
        assert!(!t1.done);
        assert_eq!(t1.phase.as_deref(), Some("Phase 1: Setup"));
        assert_eq!(t1.file_paths, ["src/retry/mod.rs"]);

        let t2 = &parsed.tasks[1];
        assert!(t2.done);

        let t3 = &parsed.tasks[2];
        assert!(!t3.parallelizable);
        assert_eq!(t3.phase.as_deref(), Some("Phase 2: Core"));
    }

    #[test]
    fn ignores_tasks_inside_html_comments() {
        let parsed = parse_tasks_md(SAMPLE);
        assert!(parsed.tasks.iter().all(|t| t.id != "T999"));
    }

    #[test]
    fn computes_parallel_layers() {
        let parsed = parse_tasks_md(SAMPLE);
        // T001,T002 parallel -> layer; T003 barrier -> singleton; T004 -> singleton.
        let layers = &parsed.layers;
        assert_eq!(layers.len(), 3);
        assert_eq!(layers[0].task_ids, ["T001", "T002"]);
        assert!(layers[0].parallel);
        assert_eq!(layers[1].task_ids, ["T003"]);
        assert!(!layers[1].parallel);
        assert_eq!(layers[2].task_ids, ["T004"]);
        assert!(!layers[2].parallel);
    }

    #[test]
    fn toggle_task_flips_checkbox() {
        let toggled = toggle_task(SAMPLE, "T001", true);
        let parsed = parse_tasks_md(&toggled);
        assert!(parsed.tasks[0].done);
        // Other lines untouched.
        assert!(parsed.tasks[1].done);
        assert!(!parsed.tasks[2].done);

        let back = toggle_task(&toggled, "T002", false);
        let parsed = parse_tasks_md(&back);
        assert!(!parsed.tasks[1].done);
    }

    #[test]
    fn toggle_unknown_task_is_noop() {
        let same = toggle_task(SAMPLE, "T404", true);
        assert_eq!(same, SAMPLE);
    }

    #[test]
    fn feature_dir_keeps_nested_branch_verbatim() {
        assert_eq!(
            feature_dir("vk/001-webhook-retries"),
            "specs/vk/001-webhook-retries"
        );
    }

    // -- pipeline gate ------------------------------------------------------

    #[test]
    fn gate_requires_pipeline_block_and_slash_command() {
        // Composed pipeline block naming a /speckit. command: triggers.
        let composed =
            "Do the thing.\n\n## Pipeline\n\n1. SpecKit: run `/speckit.constitution` first.";
        assert!(is_speckit_pipeline(composed));

        // Prose mention of /speckit. without a pipeline block: does NOT trigger.
        let prose = "Please investigate why /speckit.specify wrote to the wrong dir.";
        assert!(!is_speckit_pipeline(prose));

        // Pipeline block without any speckit command: does NOT trigger.
        let basic = "## Pipeline\n\n1. Implement the feature.\n2. Run a code review.";
        assert!(!is_speckit_pipeline(basic));
    }

    // -- spec-host resolver -------------------------------------------------

    #[test]
    fn resolver_uses_designated_host_when_present() {
        let repos = vec![
            mk_repo(uuid(1), "alpha", "Alpha", None),
            mk_repo(uuid(2), "beta", "Beta", None),
        ];
        let host = select_host_repo(Some(uuid(2)), &repos).unwrap();
        assert_eq!(host.name, "beta");
    }

    #[test]
    fn resolver_falls_back_when_designated_repo_gone() {
        let repos = vec![
            mk_repo(uuid(2), "beta", "Beta", None),
            mk_repo(uuid(1), "alpha", "Alpha", None),
        ];
        // uuid(9) is no longer part of the workspace: fall back to default order.
        let host = select_host_repo(Some(uuid(9)), &repos).unwrap();
        assert_eq!(host.name, "alpha");
    }

    #[test]
    fn resolver_picks_single_repo() {
        let repos = vec![mk_repo(uuid(3), "solo", "Zzz Solo", None)];
        let host = select_host_repo(None, &repos).unwrap();
        assert_eq!(host.name, "solo");
        assert!(select_host_repo(None, &[]).is_none());
    }

    #[test]
    fn resolver_orders_by_display_name_then_id() {
        let repos = vec![
            mk_repo(uuid(5), "b", "Same", None),
            mk_repo(uuid(4), "a", "Same", None),
            mk_repo(uuid(6), "c", "Later", None),
        ];
        // "Later" < "Same" alphabetically.
        let host = select_host_repo(None, &repos).unwrap();
        assert_eq!(host.name, "c");

        let tied = vec![
            mk_repo(uuid(5), "b", "Same", None),
            mk_repo(uuid(4), "a", "Same", None),
        ];
        // Tie on display_name: lower id wins.
        let host = select_host_repo(None, &tied).unwrap();
        assert_eq!(host.name, "a");
    }

    #[test]
    fn build_host_prefers_persisted_feature_key_over_branch() {
        let repos = vec![mk_repo(uuid(1), "alpha", "Alpha", None)];
        // First provisioning: key comes from the branch, verbatim.
        let host = build_host(None, "vk/nested/branch", None, &repos).unwrap();
        assert_eq!(host.feature_key, "vk/nested/branch");
        assert!(!host.multi_repo);

        // Branch later renamed: the persisted key stays authoritative.
        let host = build_host(Some("vk/nested/branch"), "renamed-branch", None, &repos).unwrap();
        assert_eq!(host.feature_key, "vk/nested/branch");
    }

    #[test]
    fn build_host_includes_default_working_dir_in_host_rel() {
        let repos = vec![
            mk_repo(uuid(1), "mono", "Mono", Some("backend")),
            mk_repo(uuid(2), "other", "Other", None),
        ];
        let host = build_host(None, "feat", Some(uuid(1)), &repos).unwrap();
        assert_eq!(host.host_rel, "mono/backend");
        assert!(host.multi_repo);
    }

    // -- command files ------------------------------------------------------

    fn single_ctx() -> CommandContext {
        CommandContext {
            host_rel: "myrepo".to_string(),
            feature_key: "vk/feat-1".to_string(),
            multi_repo: false,
        }
    }

    fn multi_ctx() -> CommandContext {
        CommandContext {
            host_rel: "backend".to_string(),
            feature_key: "vk/feat-1".to_string(),
            multi_repo: true,
        }
    }

    #[test]
    fn command_files_use_repo_relative_literals_in_single_repo() {
        let ctx = single_ctx();
        let specify = command_file(SpecKitStage::Specify, &ctx);
        assert!(specify.contains("`specs/vk/feat-1/spec.md`"));
        assert!(specify.contains("`.specify/memory/constitution.md`"));
        // No repo qualification, no branch derivation.
        assert!(!specify.contains("myrepo/"));
        for stage in SpecKitStage::ALL {
            let body = command_file(stage, &ctx);
            assert!(!body.contains("current git branch"));
            assert!(!body.contains("<current"));
        }
    }

    #[test]
    fn command_files_are_repo_qualified_in_multi_repo() {
        let ctx = multi_ctx();
        let specify = command_file(SpecKitStage::Specify, &ctx);
        assert!(specify.contains("`backend/specs/vk/feat-1/spec.md`"));
        assert!(specify.contains("`backend/.specify/memory/constitution.md`"));
        let plan = command_file(SpecKitStage::Plan, &ctx);
        assert!(plan.contains("`backend/specs/vk/feat-1/plan.md`"));
        for stage in SpecKitStage::ALL {
            let body = command_file(stage, &ctx);
            assert!(!body.contains("current git branch"));
            // Every specs/ occurrence must be repo-qualified.
            assert!(!body.contains("`specs/"));
            assert!(!body.contains("`.specify/"));
        }
    }

    // -- scaffold -----------------------------------------------------------

    struct TmpDir(PathBuf);
    impl TmpDir {
        fn new(tag: &str) -> Self {
            let p =
                std::env::temp_dir().join(format!("vk-speckit-test-{}-{tag}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            TmpDir(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TmpDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn scaffold_writes_host_files_and_commands() {
        let d = TmpDir::new("scaffold-basic");
        let host_root = d.path().join("backend");
        let cwd = d.path().to_path_buf(); // multi-repo: cwd = workspace root
        ensure_scaffold(&host_root, &cwd, &multi_ctx()).unwrap();

        assert!(host_root.join(CONSTITUTION_REL_PATH).exists());
        assert!(
            host_root
                .join(".specify/templates/spec-template.md")
                .exists()
        );
        assert!(
            host_root
                .join(".specify/templates/plan-template.md")
                .exists()
        );
        assert!(
            host_root
                .join(".specify/templates/tasks-template.md")
                .exists()
        );
        for stage in SpecKitStage::ALL {
            let name = slash_command(stage).trim_start_matches('/');
            assert!(cwd.join(format!(".claude/commands/{name}.md")).exists());
        }
    }

    #[test]
    fn scaffold_overwrites_stale_command_files_but_preserves_constitution() {
        let d = TmpDir::new("scaffold-overwrite");
        let host_root = d.path().join("backend");
        let cwd = d.path().to_path_buf();
        let ctx = multi_ctx();

        // Pre-seed a stale generated command file and an operator-edited
        // constitution.
        let cmd_path = cwd.join(".claude/commands/speckit.specify.md");
        write_with_parents(&cmd_path, "# stale\n\nspecs/<current git branch>/spec.md\n").unwrap();
        let constitution_path = host_root.join(CONSTITUTION_REL_PATH);
        write_with_parents(&constitution_path, "# My hand-edited constitution\n").unwrap();

        ensure_scaffold(&host_root, &cwd, &ctx).unwrap();

        // Command file overwritten with the freshly generated content.
        let cmd = std::fs::read_to_string(&cmd_path).unwrap();
        assert_eq!(cmd, command_file(SpecKitStage::Specify, &ctx));
        assert!(!cmd.contains("current git branch"));

        // Constitution untouched.
        let constitution = std::fs::read_to_string(&constitution_path).unwrap();
        assert_eq!(constitution, "# My hand-edited constitution\n");

        // Re-running with identical content is a no-op (still equal).
        ensure_scaffold(&host_root, &cwd, &ctx).unwrap();
        assert_eq!(
            std::fs::read_to_string(&cmd_path).unwrap(),
            command_file(SpecKitStage::Specify, &ctx)
        );
    }

    #[test]
    fn scaffold_refuses_foreign_feature_owner_before_writing() {
        let d = TmpDir::new("scaffold-owner-conflict");
        let host_root = d.path().join("backend");
        let cwd = d.path().to_path_buf();
        let owner = host_root.join("specs/vk/feat-1").join(FEATURE_OWNER_FILE);
        write_with_parents(&owner, "vk/a-different-task\n").unwrap();

        let error = ensure_scaffold(&host_root, &cwd, &multi_ctx()).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert!(error.to_string().contains("vk/a-different-task"));
        assert!(!cwd.join(".claude/commands/speckit.specify.md").exists());
        assert!(!host_root.join(CONSTITUTION_REL_PATH).exists());
    }

    #[test]
    fn scaffold_refreshes_same_feature_owner() {
        let d = TmpDir::new("scaffold-same-owner");
        let host_root = d.path().join("backend");
        let cwd = d.path().to_path_buf();
        let owner = host_root.join("specs/vk/feat-1").join(FEATURE_OWNER_FILE);
        write_with_parents(&owner, "vk/feat-1\n").unwrap();

        ensure_scaffold(&host_root, &cwd, &multi_ctx()).unwrap();

        assert_eq!(std::fs::read_to_string(owner).unwrap(), "vk/feat-1\n");
        assert!(cwd.join(".claude/commands/speckit.specify.md").exists());
    }

    #[test]
    fn scaffold_claims_matching_legacy_spec() {
        let d = TmpDir::new("scaffold-legacy-owner");
        let host_root = d.path().join("backend");
        let cwd = d.path().to_path_buf();
        let spec = host_root.join("specs/vk/feat-1/spec.md");
        write_with_parents(
            &spec,
            "# Feature\n\n**Task id**: `vk/feat-1`\n**Feature dir**: `specs/vk/old/`\n",
        )
        .unwrap();

        ensure_scaffold(&host_root, &cwd, &multi_ctx()).unwrap();

        assert_eq!(
            std::fs::read_to_string(spec.parent().unwrap().join(FEATURE_OWNER_FILE)).unwrap(),
            "vk/feat-1\n"
        );
    }

    #[test]
    fn scaffold_files_cover_templates_and_commands() {
        let host = host_scaffold_files();
        assert_eq!(host.len(), 4);
        assert!(
            host.iter()
                .any(|(p, _)| p == Path::new(CONSTITUTION_REL_PATH))
        );
        let cmds = command_scaffold_files(&single_ctx());
        assert_eq!(cmds.len(), SpecKitStage::ALL.len());
        assert!(
            cmds.iter()
                .any(|(p, _)| p == Path::new(".claude/commands/speckit.specify.md"))
        );
    }
}
