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
    collections::{BTreeSet, HashMap, HashSet, VecDeque},
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use api_types::Issue;
use db::models::execution_process::{ExecutionProcessRunReason, ExecutionProcessStatus};
use executors::logs::{NormalizedEntry, NormalizedEntryType};
use regex::Regex;
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
/// Text searched for (and parsed) to find earlier remediation issues.
pub const REMEDIATION_MARKER_PREFIX: &str = "<!-- vk:auto-remediation ";
/// Normalized word-set Jaccard similarity at or above which two error texts
/// are treated as the same failure (see `find_similar_issue`).
pub const SIMILARITY_THRESHOLD: f64 = 0.8;
const ERROR_SECTION_HEADING: &str = "### Error messages";

static UUID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b").unwrap()
});
// Applied in order after lowercasing. Every identifier form — UUIDs, `0x`
// literals, decimal-only words of any length, and hex words of 8+ chars
// (hashes, short SHAs, even all-letter ones like `deadbeef` — almost no
// English word is 8+ letters of a–f) — becomes the same `<id>`, so one failure
// reported with different kinds of ids still matches. Remaining digit runs
// inside words become `<n>`.
static PREFIXED_HEX_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b0x[0-9a-f]+\b").unwrap());
static DECIMAL_WORD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b[0-9]+\b").unwrap());
static HEX_WORD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b[0-9a-f]{8,}\b").unwrap());
static DIGITS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[0-9]+").unwrap());
// Two steps on purpose: an optional capture group after a lazy prefix lets
// the regex succeed without ever capturing the fingerprint.
static MARKER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<!-- vk:auto-remediation ([^>]*?)-->").unwrap());
static FINGERPRINT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bfingerprint=([0-9a-f]{16})\b").unwrap());

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

impl RemediationContext {
    /// Fingerprint of this failure's error messages; `None` when none were
    /// captured (absent evidence never matches another failure).
    pub fn fingerprint(&self) -> Option<String> {
        error_fingerprint(&self.error_messages)
    }
}

pub fn remediation_marker(
    source_workspace_id: Uuid,
    execution_process_id: Uuid,
    fingerprint: Option<&str>,
) -> String {
    let fingerprint = fingerprint
        .map(|fp| format!(" fingerprint={fp}"))
        .unwrap_or_default();
    format!(
        "{REMEDIATION_MARKER_PREFIX}source={source_workspace_id} exec={execution_process_id}{fingerprint} -->"
    )
}

/// Lowercase, replace UUIDs / digit-bearing hex words / remaining digit runs
/// with placeholders, and collapse whitespace — so errors differing only in
/// ids, timestamps, ports or line numbers normalize identically.
pub fn normalize_error_text(text: &str) -> String {
    let lower = text.to_lowercase();
    let text = UUID_RE.replace_all(&lower, "<id>");
    let text = PREFIXED_HEX_RE.replace_all(&text, "<id>");
    let text = DECIMAL_WORD_RE.replace_all(&text, "<id>");
    let text = HEX_WORD_RE.replace_all(&text, "<id>");
    let text = DIGITS_RE.replace_all(&text, "<n>");
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A stable 16-hex-char fingerprint (FNV-1a 64) of the normalized messages,
/// or `None` when there are no non-empty messages.
pub fn error_fingerprint(messages: &[String]) -> Option<String> {
    let normalized = messages
        .iter()
        .map(|m| normalize_error_text(m))
        .filter(|m| !m.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if normalized.is_empty() {
        return None;
    }
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in normalized.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    Some(format!("{hash:016x}"))
}

fn word_set(text: &str) -> BTreeSet<String> {
    normalize_error_text(text)
        .split(|c: char| !(c.is_alphanumeric() || c == '<' || c == '>' || c == '_'))
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// Jaccard similarity of the normalized word sets, in `[0, 1]`. Two empty
/// texts score 0: absent evidence is never similarity.
pub fn error_similarity(a: &str, b: &str) -> f64 {
    let (a, b) = (word_set(a), word_set(b));
    let union = a.union(&b).count();
    if union == 0 {
        return 0.0;
    }
    a.intersection(&b).count() as f64 / union as f64
}

/// The fingerprint recorded in a description's remediation marker. The outer
/// `Option` is whether a marker exists at all; the inner one is its
/// fingerprint (absent on issues filed before fingerprints existed).
pub fn parse_marker(description: &str) -> Option<Option<String>> {
    let attributes = MARKER_RE.captures(description)?.get(1)?.as_str();
    Some(
        FINGERPRINT_RE
            .captures(attributes)
            .map(|caps| caps[1].to_string()),
    )
}

/// The error text recorded in a remediation issue body: the fenced block
/// under `### Error messages`. `None` when absent or when no error was
/// captured.
pub fn issue_error_text(description: &str) -> Option<String> {
    let after =
        &description[description.find(ERROR_SECTION_HEADING)? + ERROR_SECTION_HEADING.len()..];
    let mut lines = after.lines().skip_while(|line| line.trim().is_empty());
    let opening = lines.next()?.trim_end();
    let fence_len = opening.chars().take_while(|c| *c == '`').count();
    if fence_len < 3 {
        return None;
    }
    let fence = &opening[..fence_len];
    let body: Vec<&str> = lines.take_while(|line| line.trim_end() != fence).collect();
    let text = body.join("\n");
    (!text.trim().is_empty()).then_some(text)
}

/// Mirrors the remote's active-issue rule
/// (`lower(name) NOT IN ('done', 'cancelled', 'canceled')`).
pub fn is_active_status_name(name: &str) -> bool {
    !matches!(
        name.trim().to_lowercase().as_str(),
        "done" | "cancelled" | "canceled"
    )
}

/// The first remediation issue (by the caller's ordering) whose recorded
/// errors match `messages`: equal fingerprints, or normalized word-set
/// similarity of at least [`SIMILARITY_THRESHOLD`]. Issues without the
/// remediation marker never match, and neither does a failure with no
/// captured error message.
pub fn find_similar_issue<'a>(issues: &'a [Issue], messages: &[String]) -> Option<&'a Issue> {
    let fingerprint = error_fingerprint(messages)?;
    let joined = messages.join("\n\n");
    issues.iter().find(|issue| {
        let Some(description) = issue.description.as_deref() else {
            return false;
        };
        let Some(recorded) = parse_marker(description) else {
            return false;
        };
        if recorded.as_deref() == Some(fingerprint.as_str()) {
            return true;
        }
        issue_error_text(description)
            .is_some_and(|text| error_similarity(&text, &joined) >= SIMILARITY_THRESHOLD)
    })
}

/// Comment recording another occurrence on an existing remediation issue.
pub fn compose_recurrence_comment(ctx: &RemediationContext) -> String {
    format!(
        "Another failed agent run hit a similar error, so it was added here instead of opening \
a new remediation issue.\n\n\
- Workspace: **{name}** (`{ws}`, branch `{branch}`)\n\
- Failed execution: `{exec}` (executor `{executor}`, exit code `{exit_code}`)\n\n{errors}",
        name = ctx.source_name.trim(),
        ws = ctx.source_workspace_id,
        branch = ctx.branch,
        exec = ctx.execution_process_id,
        executor = ctx.executor,
        exit_code = exit_code_label(ctx.exit_code),
        errors = error_block(&ctx.error_messages),
    )
}

fn exit_code_label(exit_code: Option<i64>) -> String {
    exit_code
        .map(|code| code.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn error_block(messages: &[String]) -> String {
    if messages.is_empty() {
        return "_No error message was captured in the chat._\n".to_string();
    }
    let joined = messages.join("\n\n");
    let fence = code_fence_for(&joined);
    format!("{fence}text\n{joined}\n{fence}\n")
}

/// Compose the remediation issue. `pipeline_block` is the composed
/// `## Pipeline` block (possibly empty) and is appended last, exactly as the
/// New Issue UI and MCP `create_issue` append it.
pub fn compose_issue(ctx: &RemediationContext, pipeline_block: &str) -> IssueDraft {
    let title = format!(
        "{AUTO_REMEDIATION_NAME_PREFIX}{} agent run failed",
        truncate_chars(ctx.source_name.trim(), MAX_NAME_CHARS)
    );

    let exit_code = exit_code_label(ctx.exit_code);

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

    body.push_str(&error_block(&ctx.error_messages));

    if let Some(variant) = &ctx.variant_fallback {
        body.push_str(&format!(
            "\n> Note: executor profile variant `{variant}` is not defined; the default variant was used.\n"
        ));
    }

    body.push('\n');
    body.push_str(&remediation_marker(
        ctx.source_workspace_id,
        ctx.execution_process_id,
        ctx.fingerprint().as_deref(),
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
        assert!(draft.description.contains(&remediation_marker(
            Uuid::nil(),
            Uuid::nil(),
            ctx().fingerprint().as_deref()
        )));
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

    fn issue(description: &str) -> Issue {
        serde_json::from_value(serde_json::json!({
            "id": Uuid::new_v4(),
            "project_id": Uuid::nil(),
            "issue_number": 1,
            "simple_id": "VAS-1",
            "status_id": Uuid::nil(),
            "title": "Auto-fix: x agent run failed",
            "description": description,
            "priority": null,
            "start_date": null,
            "target_date": null,
            "completed_at": null,
            "sort_order": 0.0,
            "parent_issue_id": null,
            "parent_issue_sort_order": null,
            "extension_metadata": {},
            "creator_user_id": null,
            "created_at": "2026-09-26T00:00:00Z",
            "updated_at": "2026-09-26T00:00:00Z"
        }))
        .unwrap()
    }

    fn with_errors(messages: &[&str]) -> RemediationContext {
        let mut ctx = ctx();
        ctx.error_messages = messages.iter().map(|m| m.to_string()).collect();
        ctx
    }

    #[test]
    fn normalization_erases_ids_numbers_and_case() {
        let a = "Request 7f3c9e2a-1b4d-4c8e-9f00-1234567890ab failed at 12:04:55 (port 8080), sha DEADBEEF1234";
        let b = "request 00000000-0000-0000-0000-000000000000 FAILED at 09:13:02 (port 3000), sha abcdef987654";
        assert_eq!(normalize_error_text(a), normalize_error_text(b));
        assert_eq!(
            error_fingerprint(&[a.to_string()]),
            error_fingerprint(&[b.to_string()])
        );
        assert_ne!(
            error_fingerprint(&[a.to_string()]),
            error_fingerprint(&["Permission denied writing config".to_string()])
        );
    }

    #[test]
    fn ids_normalize_consistently_across_lengths_and_hex_forms() {
        assert_eq!(
            normalize_error_text("Request 9999999 failed"),
            normalize_error_text("Request 10000000 failed")
        );
        assert_eq!(
            normalize_error_text("Invalid address 0xdeadbeef"),
            normalize_error_text("Invalid address 0xdeadbee1")
        );
        assert_eq!(
            normalize_error_text("commit 3f9a2c7e11 not found"),
            normalize_error_text("commit 0b1c2d3e4f not found")
        );
        assert_eq!(
            normalize_error_text("fatal: bad object 12345678"),
            normalize_error_text("fatal: bad object a1b2c3d4")
        );
        assert_eq!(
            normalize_error_text("fatal: bad object deadbeef"),
            normalize_error_text("fatal: bad object deadbee1")
        );
    }

    #[test]
    fn short_hex_like_words_are_not_erased() {
        assert!(normalize_error_text("cafe face added").contains("cafe face added"));
    }

    #[test]
    fn no_error_messages_have_no_fingerprint() {
        assert_eq!(error_fingerprint(&[]), None);
        assert_eq!(error_fingerprint(&["   ".to_string()]), None);
        assert_eq!(error_similarity("", ""), 0.0);
    }

    #[test]
    fn similarity_tolerates_one_changed_word_but_not_different_errors() {
        let a = "API Error: 529 overloaded_error while calling the model claude-opus-5-5 please retry the request later";
        let b = "API Error: 529 overloaded_error while calling the model claude-sonnet-5 please retry the request later";
        assert!(error_similarity(a, b) >= SIMILARITY_THRESHOLD);
        let c = "Git error: failed to push branch, remote rejected the update";
        assert!(error_similarity(a, c) < SIMILARITY_THRESHOLD);
    }

    #[test]
    fn marker_round_trips_with_and_without_fingerprint() {
        let draft = compose_issue(&with_errors(&["boom"]), "");
        let fingerprint = error_fingerprint(&["boom".to_string()]);
        assert!(fingerprint.is_some());
        assert_eq!(parse_marker(&draft.description), Some(fingerprint));

        let legacy = "body\n<!-- vk:auto-remediation source=a exec=b -->";
        assert_eq!(parse_marker(legacy), Some(None));
        assert_eq!(parse_marker("no marker here"), None);
    }

    #[test]
    fn issue_error_text_reads_back_the_recorded_errors() {
        let draft = compose_issue(&with_errors(&["first ``` error", "second error"]), "");
        assert_eq!(
            issue_error_text(&draft.description).as_deref(),
            Some("first ``` error\n\nsecond error")
        );
        let empty = compose_issue(&with_errors(&[]), "");
        assert_eq!(issue_error_text(&empty.description), None);
    }

    #[test]
    fn active_status_names_follow_the_remote_rule() {
        for name in ["To do", "In progress", "In review", "Backlog"] {
            assert!(is_active_status_name(name), "{name}");
        }
        for name in ["Done", " done ", "CANCELLED", "Canceled"] {
            assert!(!is_active_status_name(name), "{name}");
        }
    }

    #[test]
    fn finds_a_similar_issue_by_fingerprint_or_text() {
        let recorded = "Timeout after 30s calling 10.0.0.4";
        let recurring = vec!["Timeout after 45s calling 10.0.0.9".to_string()];
        let original = compose_issue(&with_errors(&[recorded]), "");
        let unrelated = compose_issue(&with_errors(&["Permission denied"]), "");
        let issues = vec![issue(&unrelated.description), issue(&original.description)];

        let found = find_similar_issue(&issues, &recurring);
        assert_eq!(found.map(|i| i.id), Some(issues[1].id));

        // An issue filed before fingerprints existed still matches by text.
        let fingerprint = with_errors(&[recorded]).fingerprint().unwrap();
        let legacy = original
            .description
            .replace(&format!(" fingerprint={fingerprint}"), "");
        assert_eq!(parse_marker(&legacy), Some(None));
        let legacy_issues = vec![issue(&legacy)];
        assert!(find_similar_issue(&legacy_issues, &recurring).is_some());
    }

    #[test]
    fn never_matches_without_evidence_or_marker() {
        let original = compose_issue(&with_errors(&[]), "");
        let issues = vec![issue(&original.description)];
        assert!(find_similar_issue(&issues, &[]).is_none());
        assert!(find_similar_issue(&issues, &["boom".to_string()]).is_none());

        let unmarked = vec![issue("### Error messages\n\n```text\nboom\n```")];
        assert!(find_similar_issue(&unmarked, &["boom".to_string()]).is_none());
    }

    #[test]
    fn recurrence_comment_names_the_new_occurrence() {
        let comment = compose_recurrence_comment(&with_errors(&["API Error: 500"]));
        assert!(comment.contains("**Fix login**"));
        assert!(comment.contains("branch `vk/1234-fix-login`"));
        assert!(comment.contains("exit code `1`"));
        assert!(comment.contains("```text\nAPI Error: 500\n```"));
    }
}
