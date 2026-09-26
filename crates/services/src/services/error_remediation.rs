//! Auto error remediation: turn a failed coding-agent turn into an issue with
//! pipelines attached and an unattended workspace working on it.
//!
//! This module holds the pure pieces — the trigger predicate, the loop/dedupe/
//! rate guard and the issue text. `ContainerService::finalize_task` emits an
//! [`ErrorRemediationEvent`] (no I/O on that hot path); the server's consumer
//! does the remote issue creation and workspace start.
//!
//! Safety rules (VK constitution XLI):
//! - Off unless `Config.auto_error_remediation.enabled`.
//! - Remediation workspaces are named with [`AUTO_REMEDIATION_NAME_PREFIX`] and
//!   never trigger remediation themselves, so an agent's own failure cannot
//!   recurse — the name is durable across restarts.
//! - A source workspace is remediated at most once per [`DEDUPE_WINDOW`], and
//!   launches share a global per-hour cap. A slot is reserved *before* any I/O,
//!   so a launch that fails half-way still counts and is never retried.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Mutex,
    time::{Duration, Instant},
};

use db::models::execution_process::{ExecutionProcessRunReason, ExecutionProcessStatus};
use executors::logs::{NormalizedEntry, NormalizedEntryType};
use uuid::Uuid;

use crate::services::pipelines::Pipeline;

/// Prefix of every remediation workspace (and issue) name; the durable
/// recursion guard.
pub const AUTO_REMEDIATION_NAME_PREFIX: &str = "Auto-fix: ";
/// A source workspace is remediated at most once per this window.
pub const DEDUPE_WINDOW: Duration = Duration::from_secs(24 * 60 * 60);
const RATE_WINDOW: Duration = Duration::from_secs(60 * 60);
const MAX_ERROR_MESSAGES: usize = 5;
const MAX_ERROR_CHARS: usize = 2000;
const MAX_NAME_CHARS: usize = 120;

/// Emitted by `finalize_task` for a failed coding-agent turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorRemediationEvent {
    pub workspace_id: Uuid,
    pub session_id: Uuid,
    pub execution_process_id: Uuid,
}

/// Only a coding-agent turn that genuinely failed is a chat error. User stops
/// (`Killed`), restart interrupts (`Interrupted`) and lost terminal evidence
/// (`Indeterminate`) are not, and neither are script processes.
pub fn is_remediation_trigger(
    run_reason: &ExecutionProcessRunReason,
    status: &ExecutionProcessStatus,
) -> bool {
    matches!(run_reason, ExecutionProcessRunReason::CodingAgent)
        && matches!(status, ExecutionProcessStatus::Failed)
}

pub fn is_remediation_workspace(name: Option<&str>) -> bool {
    name.is_some_and(|name| name.starts_with(AUTO_REMEDIATION_NAME_PREFIX))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardDecision {
    Proceed,
    SkipRemediationWorkspace,
    SkipDuplicate,
    SkipRateLimited,
}

#[derive(Default)]
struct GuardState {
    last_by_source: HashMap<Uuid, Instant>,
    launches: VecDeque<Instant>,
    launched: HashSet<Uuid>,
}

/// In-memory dedupe and rate guard. Restarting the server resets it; the
/// recursion guard does not depend on it (see module docs).
#[derive(Default)]
pub struct ErrorRemediationGuard {
    state: Mutex<GuardState>,
}

impl ErrorRemediationGuard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Decide whether `source` may be remediated now and, when it may,
    /// reserve the slot atomically (counting against both the dedupe window
    /// and the hourly cap) before the caller does any I/O.
    pub fn try_reserve(
        &self,
        source: Uuid,
        source_name: Option<&str>,
        now: Instant,
        max_per_hour: u32,
    ) -> GuardDecision {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());

        if is_remediation_workspace(source_name) || state.launched.contains(&source) {
            return GuardDecision::SkipRemediationWorkspace;
        }

        state
            .last_by_source
            .retain(|_, at| now.saturating_duration_since(*at) < DEDUPE_WINDOW);
        while state
            .launches
            .front()
            .is_some_and(|at| now.saturating_duration_since(*at) >= RATE_WINDOW)
        {
            state.launches.pop_front();
        }

        if state.last_by_source.contains_key(&source) {
            return GuardDecision::SkipDuplicate;
        }
        if state.launches.len() >= max_per_hour as usize {
            return GuardDecision::SkipRateLimited;
        }

        state.last_by_source.insert(source, now);
        state.launches.push_back(now);
        GuardDecision::Proceed
    }

    /// Remember a workspace this process launched, so it is never treated as
    /// a remediation source even if it were renamed.
    pub fn record_launched(&self, workspace_id: Uuid) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.launched.insert(workspace_id);
    }
}

/// Stages to enable for a remediation issue: every selected pipeline's
/// default stages — the same set the New Issue UI and MCP `create_issue`
/// start from — plus, when none of `merge_stage_ids` is already among them,
/// the first of those that exists in the selected pipelines. An unattended
/// fix is only useful once it is merged.
pub fn remediation_stage_ids(
    pipelines: &[&Pipeline],
    merge_stage_ids: &[String],
) -> HashSet<String> {
    let mut enabled: HashSet<String> = pipelines
        .iter()
        .flat_map(|p| p.stages.iter().filter(|s| s.default_enabled))
        .map(|s| s.id.clone())
        .collect();
    if !merge_stage_ids.iter().any(|id| enabled.contains(id))
        && let Some(id) = merge_stage_ids.iter().find(|id| {
            pipelines
                .iter()
                .any(|p| p.stages.iter().any(|s| &s.id == *id))
        })
    {
        enabled.insert(id.clone());
    }
    enabled
}

/// The last few error messages the chat showed for a turn, each bounded.
pub fn extract_error_messages(entries: &[NormalizedEntry]) -> Vec<String> {
    let messages: Vec<String> = entries
        .iter()
        .filter(|entry| matches!(entry.entry_type, NormalizedEntryType::ErrorMessage { .. }))
        .map(|entry| truncate_chars(entry.content.trim(), MAX_ERROR_CHARS))
        .filter(|content| !content.is_empty())
        .collect();
    let skip = messages.len().saturating_sub(MAX_ERROR_MESSAGES);
    messages.into_iter().skip(skip).collect()
}

/// What the issue body says about the failed turn.
#[derive(Debug, Clone)]
pub struct RemediationContext {
    pub source_workspace_id: Uuid,
    /// Workspace name, or its branch when unnamed.
    pub source_name: String,
    pub branch: String,
    pub execution_process_id: Uuid,
    pub executor: String,
    pub exit_code: Option<i64>,
    pub error_messages: Vec<String>,
    /// Set when the configured executor variant is undefined and the default
    /// variant is used instead.
    pub variant_fallback: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueDraft {
    pub title: String,
    pub description: String,
}

pub fn remediation_marker(source_workspace_id: Uuid, execution_process_id: Uuid) -> String {
    format!("<!-- vk:auto-remediation source={source_workspace_id} exec={execution_process_id} -->")
}

/// Compose the remediation issue. `pipeline_block` is the composed
/// `## Pipeline` block (possibly empty) and is appended last, exactly as the
/// New Issue UI and MCP `create_issue` append it.
pub fn compose_issue(ctx: &RemediationContext, pipeline_block: &str) -> IssueDraft {
    let title = format!(
        "{AUTO_REMEDIATION_NAME_PREFIX}{} agent run failed",
        truncate_chars(ctx.source_name.trim(), MAX_NAME_CHARS)
    );

    let exit_code = ctx
        .exit_code
        .map(|code| code.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let mut body = format!(
        "A coding-agent run failed in workspace **{name}** and was filed automatically for \
unattended remediation. Diagnose the root cause, fix it, test the fix, and merge it to the \
base branch.\n\n\
- Source workspace: `{ws}` (branch `{branch}`)\n\
- Failed execution: `{exec}` (executor `{executor}`, exit code `{exit_code}`)\n\n\
### Error messages\n\n",
        name = ctx.source_name.trim(),
        ws = ctx.source_workspace_id,
        branch = ctx.branch,
        exec = ctx.execution_process_id,
        executor = ctx.executor,
    );

    if ctx.error_messages.is_empty() {
        body.push_str("_No error message was captured in the chat._\n");
    } else {
        let joined = ctx.error_messages.join("\n\n");
        let fence = code_fence_for(&joined);
        body.push_str(&format!("{fence}text\n{joined}\n{fence}\n"));
    }

    if let Some(variant) = &ctx.variant_fallback {
        body.push_str(&format!(
            "\n> Note: executor profile variant `{variant}` is not defined; the default variant was used.\n"
        ));
    }

    body.push('\n');
    body.push_str(&remediation_marker(
        ctx.source_workspace_id,
        ctx.execution_process_id,
    ));

    let description =
        api_types::pipeline_block::append_pipeline_block(Some(body.clone()), pipeline_block)
            .unwrap_or(body);
    IssueDraft { title, description }
}

/// A backtick fence longer than any backtick run inside `text`, so quoted
/// error output can never close the fence early.
fn code_fence_for(text: &str) -> String {
    let mut longest = 0usize;
    let mut current = 0usize;
    for c in text.chars() {
        if c == '`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    "`".repeat((longest + 1).max(3))
}

fn truncate_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use executors::logs::NormalizedEntryError;

    use super::*;

    fn entry(entry_type: NormalizedEntryType, content: &str) -> NormalizedEntry {
        NormalizedEntry {
            timestamp: None,
            entry_type,
            content: content.to_string(),
            metadata: None,
        }
    }

    fn error(content: &str) -> NormalizedEntry {
        entry(
            NormalizedEntryType::ErrorMessage {
                error_type: NormalizedEntryError::Other,
            },
            content,
        )
    }

    fn ctx() -> RemediationContext {
        RemediationContext {
            source_workspace_id: Uuid::nil(),
            source_name: "Fix login".to_string(),
            branch: "vk/1234-fix-login".to_string(),
            execution_process_id: Uuid::nil(),
            executor: "CLAUDE_CODE".to_string(),
            exit_code: Some(1),
            error_messages: vec!["API Error: 500".to_string()],
            variant_fallback: None,
        }
    }

    #[test]
    fn only_failed_coding_agent_turns_trigger() {
        use ExecutionProcessRunReason as R;
        use ExecutionProcessStatus as S;
        assert!(is_remediation_trigger(&R::CodingAgent, &S::Failed));
        for status in [
            S::Completed,
            S::Killed,
            S::Interrupted,
            S::Indeterminate,
            S::Running,
        ] {
            assert!(
                !is_remediation_trigger(&R::CodingAgent, &status),
                "{status:?}"
            );
        }
        for reason in [R::SetupScript, R::CleanupScript, R::DevServer] {
            assert!(!is_remediation_trigger(&reason, &S::Failed), "{reason:?}");
        }
    }

    #[test]
    fn remediation_workspaces_never_trigger() {
        let guard = ErrorRemediationGuard::new();
        let now = Instant::now();
        assert_eq!(
            guard.try_reserve(Uuid::new_v4(), Some("Auto-fix: x agent run failed"), now, 3),
            GuardDecision::SkipRemediationWorkspace
        );

        let launched = Uuid::new_v4();
        guard.record_launched(launched);
        assert_eq!(
            guard.try_reserve(launched, Some("renamed"), now, 3),
            GuardDecision::SkipRemediationWorkspace
        );
    }

    #[test]
    fn a_source_is_remediated_once_per_dedupe_window() {
        let guard = ErrorRemediationGuard::new();
        let source = Uuid::new_v4();
        let now = Instant::now();
        assert_eq!(
            guard.try_reserve(source, Some("ws"), now, 10),
            GuardDecision::Proceed
        );
        assert_eq!(
            guard.try_reserve(source, Some("ws"), now + Duration::from_secs(60), 10),
            GuardDecision::SkipDuplicate
        );
        assert_eq!(
            guard.try_reserve(source, Some("ws"), now + DEDUPE_WINDOW, 10),
            GuardDecision::Proceed
        );
    }

    #[test]
    fn launches_are_capped_per_trailing_hour() {
        let guard = ErrorRemediationGuard::new();
        let now = Instant::now();
        for _ in 0..2 {
            assert_eq!(
                guard.try_reserve(Uuid::new_v4(), None, now, 2),
                GuardDecision::Proceed
            );
        }
        assert_eq!(
            guard.try_reserve(Uuid::new_v4(), None, now, 2),
            GuardDecision::SkipRateLimited
        );
        assert_eq!(
            guard.try_reserve(Uuid::new_v4(), None, now + RATE_WINDOW, 2),
            GuardDecision::Proceed
        );
    }

    #[test]
    fn zero_cap_never_launches() {
        let guard = ErrorRemediationGuard::new();
        assert_eq!(
            guard.try_reserve(Uuid::new_v4(), None, Instant::now(), 0),
            GuardDecision::SkipRateLimited
        );
    }

    #[test]
    fn rate_limited_attempt_does_not_consume_the_source() {
        let guard = ErrorRemediationGuard::new();
        let now = Instant::now();
        let source = Uuid::new_v4();
        assert_eq!(
            guard.try_reserve(source, None, now, 0),
            GuardDecision::SkipRateLimited
        );
        assert_eq!(
            guard.try_reserve(source, None, now, 1),
            GuardDecision::Proceed
        );
    }

    #[test]
    fn extracts_the_last_bounded_error_messages() {
        let long = "x".repeat(MAX_ERROR_CHARS + 10);
        let mut entries = vec![entry(NormalizedEntryType::AssistantMessage, "hello")];
        for i in 0..7 {
            entries.push(error(&format!("error {i}")));
        }
        entries.push(error("   "));
        entries.push(error(&long));

        let messages = extract_error_messages(&entries);
        assert_eq!(messages.len(), MAX_ERROR_MESSAGES);
        assert_eq!(messages[0], "error 3");
        assert_eq!(messages[4].chars().count(), MAX_ERROR_CHARS + 1);
        assert!(messages[4].ends_with('…'));
    }

    #[test]
    fn issue_carries_prefix_context_marker_and_trailing_block() {
        let block = "<!-- vk:pipeline:start -->\n## Pipeline: X\n<!-- vk:pipeline:end -->";
        let draft = compose_issue(&ctx(), block);

        assert_eq!(draft.title, "Auto-fix: Fix login agent run failed");
        assert!(is_remediation_workspace(Some(&draft.title)));
        assert!(draft.description.contains("branch `vk/1234-fix-login`"));
        assert!(draft.description.contains("exit code `1`"));
        assert!(draft.description.contains("```text\nAPI Error: 500\n```"));
        assert!(
            draft
                .description
                .contains(&remediation_marker(Uuid::nil(), Uuid::nil()))
        );
        assert!(draft.description.ends_with(&format!("\n\n{block}")));
    }

    #[test]
    fn issue_notes_missing_errors_and_variant_fallback() {
        let mut ctx = ctx();
        ctx.error_messages.clear();
        ctx.exit_code = None;
        ctx.variant_fallback = Some("PROALIGN".to_string());
        let draft = compose_issue(&ctx, "");

        assert!(draft.description.contains("_No error message was captured"));
        assert!(draft.description.contains("exit code `unknown`"));
        assert!(
            draft
                .description
                .contains("variant `PROALIGN` is not defined")
        );
        assert!(draft.description.ends_with("-->"));
    }

    #[test]
    fn error_text_with_backticks_cannot_close_the_fence() {
        let mut ctx = ctx();
        ctx.error_messages = vec!["bad ``` fence".to_string()];
        let draft = compose_issue(&ctx, "");
        assert!(draft.description.contains("````text\nbad ``` fence\n````"));
    }

    #[test]
    fn long_names_are_truncated_in_the_title() {
        let mut ctx = ctx();
        ctx.source_name = "n".repeat(500);
        let draft = compose_issue(&ctx, "");
        assert!(draft.title.chars().count() < 200);
    }

    fn pipeline(id: &str, stages: &[(&str, bool)]) -> Pipeline {
        Pipeline {
            id: id.to_string(),
            name: id.to_string(),
            description: None,
            stages: stages
                .iter()
                .map(
                    |(id, default_enabled)| crate::services::pipelines::PipelineStep {
                        id: id.to_string(),
                        label: id.to_string(),
                        prompt_fragment: format!("Do {id}."),
                        default_enabled: *default_enabled,
                        heavy: false,
                    },
                )
                .collect(),
        }
    }

    fn merge_ids() -> Vec<String> {
        vec!["pr-and-merge".to_string(), "merge".to_string()]
    }

    #[test]
    fn default_merge_stage_keeps_the_ui_default_selection() {
        // Mirrors this deployment's customized SpecKit pipeline, where
        // "open and merge PR" is already a default stage.
        let wikillm = pipeline("wikillm", &[("spec", true), ("merge", false)]);
        let speckit = pipeline(
            "speckit",
            &[
                ("speckit-implement", true),
                ("merge", false),
                ("pr-and-merge", true),
            ],
        );
        let enabled = remediation_stage_ids(&[&wikillm, &speckit], &merge_ids());
        let expected: HashSet<String> = ["spec", "speckit-implement", "pr-and-merge"]
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(enabled, expected);
    }

    #[test]
    fn a_merge_stage_is_enabled_when_no_default_merges() {
        let wikillm = parse_bundled(
            "wikillm",
            include_str!("../../../../assets/pipelines/wikillm.toml"),
        );
        let speckit = parse_bundled(
            "speckit",
            include_str!("../../../../assets/pipelines/speckit.toml"),
        );
        let enabled = remediation_stage_ids(&[&wikillm, &speckit], &merge_ids());

        assert!(enabled.contains("merge"));
        assert!(enabled.contains("speckit-implement"));
        assert!(enabled.contains("enrich-knowledge"));
        assert!(!enabled.contains("pr"));
        assert!(!enabled.contains("wait-for-approval"));
    }

    #[test]
    fn missing_merge_stages_add_nothing() {
        let basic = pipeline("basic", &[("spec", true)]);
        let enabled = remediation_stage_ids(&[&basic], &merge_ids());
        assert_eq!(enabled, ["spec".to_string()].into_iter().collect());
        assert!(remediation_stage_ids(&[&basic], &[]).contains("spec"));
    }

    fn parse_bundled(id: &str, raw: &str) -> Pipeline {
        crate::services::pipelines::parse_pipeline(id, raw).unwrap()
    }
}
