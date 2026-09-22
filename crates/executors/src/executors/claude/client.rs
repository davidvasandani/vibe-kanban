use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use workspace_utils::approvals::{ApprovalStatus, QuestionStatus};

use super::types::PermissionMode;
use crate::{
    approvals::{ExecutorApprovalError, ExecutorApprovalService},
    env::RepoContext,
    executors::{
        ExecutorError,
        claude::{
            ClaudeJson, ClaudeMcpInventory,
            types::{
                PermissionResult, PermissionUpdate, PermissionUpdateDestination,
                PermissionUpdateType,
            },
        },
        codex::client::LogWriter,
    },
};

const EXIT_PLAN_MODE_NAME: &str = "ExitPlanMode";
const ASK_USER_QUESTION_NAME: &str = "AskUserQuestion";
pub const AUTO_APPROVE_CALLBACK_ID: &str = "AUTO_APPROVE_CALLBACK_ID";
pub const STOP_GIT_CHECK_CALLBACK_ID: &str = "STOP_GIT_CHECK_CALLBACK_ID";
/// PreToolUse callback id used to deny `ScheduleWakeup` calls (VAS-283).
pub const DENY_SCHEDULE_WAKEUP_CALLBACK_ID: &str = "DENY_SCHEDULE_WAKEUP_CALLBACK_ID";
/// Reason surfaced to the agent when it tries to schedule a wake-up under a VK
/// execution. VK has no supervising loop and reaps the turn's process at turn
/// end (see `wiki/agent-process-lifecycle.md`), so a harness wake-up timer
/// never fires and any work parked on it is silently dropped. The message tells
/// the agent to continue inline instead of ending its turn.
pub const SCHEDULE_WAKEUP_DENY_REASON: &str = "Scheduled wake-ups are not supported for Vibe Kanban executions: this turn's process is terminated when the turn ends, so the wake-up would never fire and any work you defer to it would be silently dropped. Do the work now in this turn instead of parking it on a wake-up, or leave a follow-up message to continue after the turn completes.";

/// PreToolUse hook response that denies a `ScheduleWakeup` call with an
/// actionable reason (VAS-283). Extracted so it can be unit-tested directly.
pub fn schedule_wakeup_deny_response() -> serde_json::Value {
    serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": SCHEDULE_WAKEUP_DENY_REASON,
        }
    })
}

/// PreToolUse callback id used to deny `Bash` calls that request a background
/// spawn (`run_in_background: true`).
///
/// Registered on a `^Bash$` matcher rather than on a dedicated tool name
/// because the control is *parameter*-granular: `Bash` itself must keep
/// working. See [`super::BACKGROUND_POLLER_TOOLS`] for why denying the
/// background-polling tool names alone is not sufficient.
pub const DENY_BACKGROUND_BASH_CALLBACK_ID: &str = "DENY_BACKGROUND_BASH_CALLBACK_ID";
/// Reason surfaced to the agent when it tries to start a background process
/// inside a VK turn. A turn is one OS process group and VK reaps it at turn end
/// (see `wiki/agent-process-lifecycle.md`), so anything started with
/// `run_in_background` dies with the turn and the output the agent planned to
/// poll is silently lost. The message names the supported replacement
/// (`spawn_poller`, which runs the command in its own surviving process group)
/// and — mirroring [`SCHEDULE_WAKEUP_DENY_REASON`] — tells the agent to keep
/// working rather than park its turn.
pub const BACKGROUND_BASH_DENY_REASON: &str = "Background processes are not supported inside a Vibe Kanban turn: this turn's process group is terminated when the turn ends, so anything started with run_in_background is reaped with it and any output you meant to poll is silently lost. Run the command in the foreground instead. If it genuinely needs to outlive this turn, use the `spawn_poller` MCP tool, which runs it in its own process group that survives the turn and is visible in the workspace UI. Supply stop_command (exit zero to stop), a positive timeout_secs, or both. Either way, keep working in this turn instead of waiting on a background process.";

/// True only when a `PreToolUse` hook input explicitly asks for a background
/// `Bash` spawn.
///
/// Deliberately conservative: an absent, non-boolean, or otherwise malformed
/// `tool_input` yields `false` (i.e. allow). An over-broad deny here would
/// break every `Bash` call in every Claude execution, so ambiguity resolves to
/// the permissive answer.
pub fn is_background_bash_input(input: &serde_json::Value) -> bool {
    input
        .get("tool_input")
        .and_then(|tool_input| tool_input.get("run_in_background"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

/// PreToolUse hook response that denies a background `Bash` spawn with an
/// actionable reason naming `spawn_poller`. Extracted so it can be unit-tested
/// directly.
pub fn background_bash_deny_response() -> serde_json::Value {
    serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": BACKGROUND_BASH_DENY_REASON,
        }
    })
}

/// Reason surfaced to the agent when it asks to run a foreground wait loop with
/// no exit guarantee.
///
/// Mirrors [`BACKGROUND_BASH_DENY_REASON`]: it names `spawn_poller` and states
/// what that replacement requires, because a denial that removes a capability
/// without offering the replacement converts a hang into a stall.
pub const UNBOUNDED_WAIT_DENY_REASON: &str = "Unbounded waiting is not supported inside a Vibe Kanban turn: this command loops until a condition becomes true, and if that condition never becomes true the turn blocks indefinitely instead of failing (one such loop held a turn for about an hour). Either bound the wait yourself — wrap it in `timeout`, cap the iterations with a counter, or use a `for` loop — or, if the thing you are waiting for may take longer than this turn, use the `spawn_poller` MCP tool, which runs the command in its own process group that survives the turn and is visible in the workspace UI. It requires stop_command (exit zero to stop), a positive timeout_secs, or both. Either way, keep working in this turn instead of blocking on the wait.";

/// Shell keywords that begin a loop with no inherent iteration bound.
const UNBOUNDED_LOOP_KEYWORDS: &[&str] = &["while", "until"];

/// Bare tokens that are evidence the loop already terminates.
///
/// - `SECONDS` is the bash builtin used to build a deadline
///   (`while [ $SECONDS -lt 60 ]`).
/// - `read` makes the loop consume a finite input stream
///   (`while read -r line; do …; sleep 1; done`), which ends when the stream
///   does. Rate-limited `while read` loops are ordinary and were previously
///   refused whenever they contained a `sleep`.
const WAIT_BOUND_TOKENS: &[&str] = &["SECONDS", "read"];

/// Arithmetic expansion — the marker for an attempt counter, as in
/// `n=0; while [ $n -lt 5 ]; do …; n=$((n+1)); done`.
///
/// Matched as a **substring**, since `$((` contains no word characters.
///
/// This replaced a set of bare comparison operators (`-lt`, `-gt`, …). Those
/// matched anywhere in the command, including inside the *condition* of a
/// polling loop — `until [ $(grep -c ready "$f") -gt 0 ]; do sleep 5; done` was
/// read as "bounded" purely because it compares numbers, even though it is
/// semantically the incident command. An increment is evidence of a counter in
/// a way a comparison is not, and `$((` does not collide with the plain `$(`
/// command substitution a polling condition uses.
const ARITHMETIC_EXPANSION: &str = "$((";

/// Whether `haystack` contains `needle` delimited by non-word characters, so
/// `sleep` does not match `sleeping` and `while` does not match `awhile`.
///
/// `-` is treated as part of a token so that a flag such as `--timeout` is a
/// single token which does not match the bare `timeout` command.
fn contains_token(haystack: &str, needle: &str) -> bool {
    let is_word = |c: char| c.is_alphanumeric() || c == '_' || c == '-';
    haystack.match_indices(needle).any(|(start, matched)| {
        let end = start + matched.len();
        haystack[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !is_word(c))
            && haystack[end..].chars().next().is_none_or(|c| !is_word(c))
    })
}

/// Whether `needle` appears as a shell *command keyword* — at a token boundary
/// **and** in command position — rather than inside a string or an argument.
///
/// [`contains_token`] alone is not enough for the loop keywords. `until` and
/// `while` are ordinary English words, so a plain token match refuses commands
/// that merely mention them:
/// `echo "retrying until ready"; sleep 2` has both a loop keyword and a sleep
/// and would be denied, which FR-5/FR-6 forbid. Requiring command position —
/// start of input, or after a separator or `do`/`then`/`else` — keeps the
/// keyword's *shell* meaning and drops the prose.
fn contains_command_keyword(haystack: &str, needle: &str) -> bool {
    let is_word = |c: char| c.is_alphanumeric() || c == '_' || c == '-';
    haystack.match_indices(needle).any(|(start, matched)| {
        let end = start + matched.len();
        if haystack[end..].chars().next().is_some_and(is_word) {
            return false;
        }
        // Trim only blanks, never the newline: `trim_end()` would strip it and
        // make the `'\n'` separator below unreachable, so a loop keyword
        // starting a *new line* would not read as command position. A
        // multi-line script is the ordinary way to write a wait loop, so that
        // silently disabled the guard for the shape it exists to catch.
        let before = haystack[..start].trim_end_matches([' ', '\t', '\r']);
        before.is_empty()
            || before.ends_with([';', '&', '|', '(', '{', '\n'])
            || ["do", "then", "else"]
                .iter()
                .any(|kw| contains_token(before, kw) && before.ends_with(kw))
    })
}

/// True only when a `PreToolUse` hook input is a foreground command whose
/// recognisable purpose is to wait indefinitely for a condition.
///
/// The rule is syntactic and deliberately narrow — deny when the command has an
/// unbounded loop keyword **and** a `sleep` **and** no bounding marker, or when
/// it leads with `watch`. `for` loops are never denied; they are bounded by
/// construction. Requiring `sleep` is what keeps `while read line; do … done`
/// and ordinary long builds out of scope: the target is the wait-loop *shape*,
/// not duration.
///
/// Conservative in the same way as [`is_background_bash_input`]: an absent,
/// non-string or otherwise malformed `command` yields `false` (allow). `Bash`
/// is the workhorse tool and an over-broad deny here would break every Claude
/// execution, so ambiguity resolves permissively.
///
/// # Known limits
///
/// - **Writing** a script that contains a wait loop (a heredoc into a file) is
///   refused as though the loop were being run. Left as-is: agents create files
///   with the `Write`/`Edit` tools, so the heredoc path is rare, and detecting
///   redirection reliably would mean parsing the shell.
/// - A wait loop assembled through a variable or `eval` does not match. The
///   guard targets an agent that stalls in good faith, not an adversary; the
///   per-command bound still applies.
/// - A busy loop with no `sleep` (`while true; do :; done`) is allowed by this
///   predicate and bounded only by the command timeout.
/// - A bound marker anywhere in the command shadows the whole loop, so an
///   unbounded wait that happens to contain arithmetic or a `read` elsewhere is
///   allowed. Markers signal *intent to bound*; they are not proof of one, and
///   ambiguity resolves permissively by design.
///
/// # Scope (Constitution IX)
///
/// This control is **Claude-only, by decision rather than by oversight**.
///
/// - *Codex* has no `PreToolUse` equivalent to attach a refusal to — that is
///   why the background block had to be delivered as prose in
///   `POLLER_DEVELOPER_INSTRUCTIONS` (`executors::codex`) instead. Its
///   per-command deadline has since been **verified absent as a VK-settable
///   identifier**: the default is a hard-coded `const` upstream with no config
///   key or environment variable, so no equivalent of the Claude environment
///   bound ships either. The evidence is recorded next to
///   `features.unified_exec` in `executors::codex`. Codex is also structurally
///   far less exposed — every exec is bounded, defaulting to ten seconds.
/// - *Grok* reaches the shell over ACP, whose terminal capability VK never
///   advertises; that verified absence is already recorded in
///   `wiki/vk-pollers.md` and is unchanged here.
pub fn is_unbounded_wait_command(input: &serde_json::Value) -> bool {
    let Some(command) = input
        .get("tool_input")
        .and_then(|tool_input| tool_input.get("command"))
        .and_then(|value| value.as_str())
    else {
        return false;
    };

    // `watch` repeats forever by definition. Only as the *leading* token:
    // matching it anywhere would hit `cargo watch` and `--watch` flags, which
    // are legitimate dev-server shapes VK handles elsewhere.
    if command
        .split_whitespace()
        .next()
        .is_some_and(|first| first == "watch")
    {
        return true;
    }

    let has_unbounded_loop = UNBOUNDED_LOOP_KEYWORDS
        .iter()
        .any(|keyword| contains_command_keyword(command, keyword));
    let waits = contains_token(command, "sleep");
    // `timeout` counts only in *command position*. Matched as a bare token it
    // also fired on any path or variable containing the word, so
    // `until test -f /tmp/timeout.flag; do sleep 5; done` read as bounded
    // because of the file it was polling for.
    let is_bounded = contains_command_keyword(command, "timeout")
        || WAIT_BOUND_TOKENS
            .iter()
            .any(|marker| contains_token(command, marker))
        || command.contains(ARITHMETIC_EXPANSION);

    has_unbounded_loop && waits && !is_bounded
}

/// PreToolUse hook response that denies an unbounded foreground wait with an
/// actionable reason naming `spawn_poller`. Extracted so it can be unit-tested
/// directly.
pub fn unbounded_wait_deny_response() -> serde_json::Value {
    serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": UNBOUNDED_WAIT_DENY_REASON,
        }
    })
}

// Prefix for denial messages from the user, mirrors claude code CLI behavior
const TOOL_DENY_PREFIX: &str = "The user doesn't want to proceed with this tool use. The tool use was rejected (eg. if it was a file edit, the new_string was NOT written to the file). To tell you how to proceed, the user said: ";

/// Claude Agent client with control protocol support
pub struct ClaudeAgentClient {
    log_writer: LogWriter,
    approvals: Option<Arc<dyn ExecutorApprovalService>>,
    auto_approve: bool, // true when approvals is None
    repo_context: RepoContext,
    commit_reminder_prompt: String,
    cancel: CancellationToken,
    mcp_inventory: Arc<ClaudeMcpInventory>,
}

impl ClaudeAgentClient {
    /// Create a new client with optional approval service
    pub(crate) fn new(
        log_writer: LogWriter,
        approvals: Option<Arc<dyn ExecutorApprovalService>>,
        repo_context: RepoContext,
        commit_reminder_prompt: String,
        cancel: CancellationToken,
        mcp_inventory: Arc<ClaudeMcpInventory>,
    ) -> Arc<Self> {
        let auto_approve = approvals.is_none();
        Arc::new(Self {
            log_writer,
            approvals,
            auto_approve,
            repo_context,
            commit_reminder_prompt,
            cancel,
            mcp_inventory,
        })
    }

    async fn handle_approval(
        &self,
        tool_use_id: String,
        tool_name: String,
        tool_input: serde_json::Value,
    ) -> Result<PermissionResult, ExecutorError> {
        let approval_service = self
            .approvals
            .as_ref()
            .ok_or(ExecutorApprovalError::ServiceUnavailable)?;

        let approval_id = match approval_service.create_tool_approval(&tool_name).await {
            Ok(id) => id,
            Err(err) => {
                self.handle_approval_error(&tool_name, &tool_use_id, &err)
                    .await?;
                return Err(err.into());
            }
        };

        let _ = self
            .log_writer
            .log_raw(&serde_json::to_string(&ClaudeJson::ApprovalRequested {
                tool_call_id: tool_use_id.clone(),
                tool_name: tool_name.clone(),
                approval_id: approval_id.clone(),
            })?)
            .await;

        let status = match approval_service
            .wait_tool_approval(&approval_id, self.cancel.clone())
            .await
        {
            Ok(s) => s,
            Err(err) => {
                self.handle_approval_error(&tool_name, &tool_use_id, &err)
                    .await?;
                return Err(err.into());
            }
        };

        self.log_writer
            .log_raw(&serde_json::to_string(&ClaudeJson::ApprovalResponse {
                tool_call_id: tool_use_id.clone(),
                tool_name: tool_name.clone(),
                approval_status: status.clone(),
            })?)
            .await?;

        match status {
            ApprovalStatus::Approved => {
                if tool_name == EXIT_PLAN_MODE_NAME {
                    Ok(PermissionResult::Allow {
                        updated_input: tool_input,
                        updated_permissions: Some(vec![PermissionUpdate {
                            update_type: PermissionUpdateType::SetMode,
                            mode: Some(PermissionMode::BypassPermissions),
                            destination: Some(PermissionUpdateDestination::Session),
                            rules: None,
                            behavior: None,
                            directories: None,
                        }]),
                    })
                } else {
                    Ok(PermissionResult::Allow {
                        updated_input: tool_input,
                        updated_permissions: None,
                    })
                }
            }
            ApprovalStatus::Denied { reason } => Ok(PermissionResult::Deny {
                message: format!("{}{}", TOOL_DENY_PREFIX, reason.unwrap_or_default()),
                interrupt: Some(false),
            }),
            ApprovalStatus::TimedOut => Ok(PermissionResult::Deny {
                message: "Approval request timed out".to_string(),
                interrupt: Some(true),
            }),
            ApprovalStatus::Pending => Ok(PermissionResult::Deny {
                message: "Approval still pending (unexpected)".to_string(),
                interrupt: Some(false),
            }),
        }
    }

    async fn handle_question(
        &self,
        tool_use_id: String,
        tool_name: String,
        tool_input: serde_json::Value,
    ) -> Result<PermissionResult, ExecutorError> {
        let approval_service = self
            .approvals
            .as_ref()
            .ok_or(ExecutorApprovalError::ServiceUnavailable)?;

        let question_count = tool_input
            .get("questions")
            .and_then(|q| q.as_array())
            .map(|a| a.len())
            .unwrap_or(1);

        let approval_id = match approval_service
            .create_question_approval(&tool_name, question_count)
            .await
        {
            Ok(id) => id,
            Err(err) => {
                self.handle_question_error(&tool_use_id, &tool_name, &err)
                    .await?;
                return Err(err.into());
            }
        };

        let _ = self
            .log_writer
            .log_raw(&serde_json::to_string(&ClaudeJson::ApprovalRequested {
                tool_call_id: tool_use_id.clone(),
                tool_name: tool_name.clone(),
                approval_id: approval_id.clone(),
            })?)
            .await;

        let status = match approval_service
            .wait_question_answer(&approval_id, self.cancel.clone())
            .await
        {
            Ok(s) => s,
            Err(err) => {
                self.handle_question_error(&tool_use_id, &tool_name, &err)
                    .await?;
                return Err(err.into());
            }
        };

        self.log_writer
            .log_raw(&serde_json::to_string(&ClaudeJson::QuestionResponse {
                tool_call_id: tool_use_id.clone(),
                tool_name: tool_name.clone(),
                question_status: status.clone(),
            })?)
            .await?;

        match status {
            QuestionStatus::Answered { answers } => {
                let answers_map: serde_json::Map<String, serde_json::Value> = answers
                    .iter()
                    .map(|qa| {
                        (
                            qa.question.clone(),
                            serde_json::Value::String(qa.answer.join(", ")),
                        )
                    })
                    .collect();
                let mut updated = tool_input.clone();
                if let Some(obj) = updated.as_object_mut() {
                    obj.insert(
                        "answers".to_string(),
                        serde_json::Value::Object(answers_map),
                    );
                }
                Ok(PermissionResult::Allow {
                    updated_input: updated,
                    updated_permissions: None,
                })
            }
            QuestionStatus::TimedOut => Ok(PermissionResult::Deny {
                message: "Question request timed out".to_string(),
                interrupt: Some(true),
            }),
        }
    }

    async fn handle_approval_error(
        &self,
        tool_name: &str,
        tool_use_id: &str,
        err: &ExecutorApprovalError,
    ) -> Result<(), ExecutorError> {
        if !matches!(err, ExecutorApprovalError::Cancelled) {
            tracing::error!(
                "Claude approval failed for tool={} call_id={}: {err}",
                tool_name,
                tool_use_id
            );
        }
        let _ = self
            .log_writer
            .log_raw(&serde_json::to_string(&ClaudeJson::ApprovalResponse {
                tool_call_id: tool_use_id.to_string(),
                tool_name: tool_name.to_string(),
                approval_status: ApprovalStatus::Denied {
                    reason: Some(format!("Approval service error: {err}")),
                },
            })?)
            .await;
        Ok(())
    }

    async fn handle_question_error(
        &self,
        tool_use_id: &str,
        tool_name: &str,
        err: &ExecutorApprovalError,
    ) -> Result<(), ExecutorError> {
        if !matches!(err, ExecutorApprovalError::Cancelled) {
            tracing::error!("Claude question failed {err}",);
        }
        let _ = self
            .log_writer
            .log_raw(&serde_json::to_string(&ClaudeJson::QuestionResponse {
                tool_call_id: tool_use_id.to_string(),
                tool_name: tool_name.to_string(),
                question_status: QuestionStatus::TimedOut,
            })?)
            .await;
        Ok(())
    }

    pub async fn on_can_use_tool(
        &self,
        tool_name: String,
        input: serde_json::Value,
        _permission_suggestions: Option<Vec<PermissionUpdate>>,
        tool_use_id: Option<String>,
    ) -> Result<PermissionResult, ExecutorError> {
        if tool_name == ASK_USER_QUESTION_NAME {
            if let Some(latest_tool_use_id) = tool_use_id {
                return self
                    .handle_question(latest_tool_use_id, tool_name, input)
                    .await;
            } else {
                tracing::warn!("AskUserQuestion without tool_use_id, cannot route to approval");
                return Ok(PermissionResult::Deny {
                    message:
                        "AskUserQuestion requires user interaction but no tool_use_id was provided"
                            .to_string(),
                    interrupt: Some(false),
                });
            }
        }
        if self.auto_approve {
            Ok(PermissionResult::Allow {
                updated_input: input,
                updated_permissions: None,
            })
        } else if let Some(latest_tool_use_id) = tool_use_id {
            self.handle_approval(latest_tool_use_id, tool_name, input)
                .await
        } else {
            // Auto approve tools with no matching tool_use_id
            // tool_use_id is undocumented so this may not be possible
            tracing::warn!(
                "No tool_use_id available for tool '{}', cannot request approval",
                tool_name
            );
            Ok(PermissionResult::Allow {
                updated_input: input,
                updated_permissions: None,
            })
        }
    }

    pub async fn on_hook_callback(
        &self,
        callback_id: String,
        input: serde_json::Value,
        _tool_use_id: Option<String>,
    ) -> Result<serde_json::Value, ExecutorError> {
        // Stop hook git check - uses `decision` (approve/block) and `reason` fields
        if callback_id == STOP_GIT_CHECK_CALLBACK_ID {
            // The execution was interrupted; don't block the stop to ask
            // Claude to keep working.
            if self.cancel.is_cancelled() {
                return Ok(serde_json::json!({"decision": "approve"}));
            }
            if input
                .get("stop_hook_active")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                return Ok(serde_json::json!({"decision": "approve"}));
            }
            let status = self.repo_context.check_uncommitted_changes().await;
            return Ok(if status.is_empty() {
                serde_json::json!({"decision": "approve"})
            } else {
                serde_json::json!({
                    "decision": "block",
                    "reason": format!("{}\n{}", self.commit_reminder_prompt, status)
                })
            });
        }

        // Deny scheduled wake-ups (VAS-283). Checked *before* `auto_approve` so
        // it also denies in bypass/auto mode — the exact mode the incident
        // occurred in — instead of the auto-approve branch turning it into an
        // "allow" that lets the agent park its turn on a wake-up that never
        // fires.
        if callback_id == DENY_SCHEDULE_WAKEUP_CALLBACK_ID {
            return Ok(schedule_wakeup_deny_response());
        }

        // Deny *background* `Bash` spawns, for the same reason and in the same
        // place: checked before `auto_approve` so it also fires in
        // bypass/yolo mode. Unlike `ScheduleWakeup` this is a parameter-level
        // rule — only `run_in_background: true` is denied, and anything else
        // (absent, `false`, or malformed `tool_input`) falls through to the
        // normal decision path below. `Bash` is the workhorse tool; an
        // over-broad deny here would break every Claude execution.
        //
        // The same callback also carries the unbounded-wait refusal: both are
        // parameter-level rules on `Bash`, the hook already fires on every
        // `Bash` call in every mode, and reusing it keeps one chokepoint and
        // leaves `get_hooks` untouched. Background is tested first so its more
        // specific message wins when a call is both. Anything that matches
        // neither must *fall through* to the normal decision path below — this
        // block deliberately does not return a default.
        if callback_id == DENY_BACKGROUND_BASH_CALLBACK_ID {
            if is_background_bash_input(&input) {
                return Ok(background_bash_deny_response());
            }
            if is_unbounded_wait_command(&input) {
                return Ok(unbounded_wait_deny_response());
            }
        }

        if self.auto_approve {
            Ok(serde_json::json!({
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "allow",
                    "permissionDecisionReason": "Auto-approved by SDK"
                }
            }))
        } else {
            match callback_id.as_str() {
                AUTO_APPROVE_CALLBACK_ID => Ok(serde_json::json!({
                    "hookSpecificOutput": {
                        "hookEventName": "PreToolUse",
                        "permissionDecision": "allow",
                        "permissionDecisionReason": "Approved by SDK"
                    }
                })),
                // A *foreground* `Bash` call that reached this hook. Return no
                // permission decision at all so the mode's own catch-all
                // matcher decides exactly as it did before this hook existed
                // (auto-approve in plan mode, `tool_approval` in approvals
                // mode). Forwarding to can_use_tool here instead would turn
                // every plan-mode `Bash` into a user prompt.
                DENY_BACKGROUND_BASH_CALLBACK_ID => Ok(serde_json::json!({})),
                _ => {
                    // Hook callbacks is only used to forward approval requests to can_use_tool.
                    // This works because `ask` decision in hook callback triggers a can_use_tool request
                    // https://docs.claude.com/en/api/agent-sdk/permissions#permission-flow-diagram
                    Ok(serde_json::json!({
                        "hookSpecificOutput": {
                            "hookEventName": "PreToolUse",
                            "permissionDecision": "ask",
                            "permissionDecisionReason": "Forwarding to canusetool service"
                        }
                    }))
                }
            }
        }
    }

    pub async fn log_message(&self, line: &str) -> Result<(), ExecutorError> {
        if let Ok(ClaudeJson::System {
            subtype: Some(subtype),
            tools: Some(tools),
            mcp_servers,
            ..
        }) = serde_json::from_str::<ClaudeJson>(line)
            && subtype == "init"
        {
            self.mcp_inventory.observe_tools(&tools, &mcp_servers).await;
        }
        self.log_writer.log_raw(line).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        approvals::NoopExecutorApprovalService, env::RepoContext,
        executors::codex::client::LogWriter,
    };

    fn auto_approve_client() -> Arc<ClaudeAgentClient> {
        // `approvals: None` => auto_approve = true, the mode the VAS-283
        // incident occurred in.
        ClaudeAgentClient::new(
            LogWriter::new(tokio::io::sink()),
            None,
            RepoContext::default(),
            String::new(),
            CancellationToken::new(),
            Arc::new(ClaudeMcpInventory::default()),
        )
    }

    /// A client with an approval service attached (`auto_approve = false`) —
    /// the shape used by both plan mode and approvals mode.
    fn approval_client() -> Arc<ClaudeAgentClient> {
        ClaudeAgentClient::new(
            LogWriter::new(tokio::io::sink()),
            Some(Arc::new(NoopExecutorApprovalService)),
            RepoContext::default(),
            String::new(),
            CancellationToken::new(),
            Arc::new(ClaudeMcpInventory::default()),
        )
    }

    fn permission_decision(resp: &serde_json::Value) -> Option<&str> {
        resp["hookSpecificOutput"]["permissionDecision"].as_str()
    }

    fn bash_hook_input(tool_input: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": tool_input,
        })
    }

    #[test]
    fn schedule_wakeup_deny_response_is_a_deny() {
        let resp = schedule_wakeup_deny_response();
        let out = &resp["hookSpecificOutput"];
        assert_eq!(out["hookEventName"], "PreToolUse");
        assert_eq!(out["permissionDecision"], "deny");
        assert_eq!(out["permissionDecisionReason"], SCHEDULE_WAKEUP_DENY_REASON);
    }

    #[tokio::test]
    async fn schedule_wakeup_callback_denies_even_in_auto_approve() {
        let client = auto_approve_client();
        let resp = client
            .on_hook_callback(
                DENY_SCHEDULE_WAKEUP_CALLBACK_ID.to_string(),
                serde_json::json!({}),
                None,
            )
            .await
            .expect("callback ok");
        // Denied despite auto_approve — the deny is checked before the
        // auto-approve short-circuit, so a parked turn cannot slip through.
        assert_eq!(resp["hookSpecificOutput"]["permissionDecision"], "deny");
    }

    #[test]
    fn background_bash_deny_response_is_a_deny_naming_spawn_poller() {
        let resp = background_bash_deny_response();
        let out = &resp["hookSpecificOutput"];
        assert_eq!(out["hookEventName"], "PreToolUse");
        assert_eq!(out["permissionDecision"], "deny");
        assert_eq!(out["permissionDecisionReason"], BACKGROUND_BASH_DENY_REASON);
        // The denial must name its replacement, otherwise the agent has no
        // supported way to do what it was trying to do.
        assert!(
            BACKGROUND_BASH_DENY_REASON.contains("spawn_poller"),
            "deny reason must name the replacement tool"
        );
    }

    #[tokio::test]
    async fn background_bash_denied_even_in_auto_approve() {
        let client = auto_approve_client();
        let resp = client
            .on_hook_callback(
                DENY_BACKGROUND_BASH_CALLBACK_ID.to_string(),
                bash_hook_input(serde_json::json!({
                    "command": "sleep 600",
                    "run_in_background": true,
                })),
                None,
            )
            .await
            .expect("callback ok");
        // Denied despite auto_approve — the check sits before the auto-approve
        // short-circuit, so bypass/yolo (the mode incidents occur in) is
        // covered.
        assert_eq!(permission_decision(&resp), Some("deny"));
    }

    /// **Release gate for the background-`Bash` rule.** An over-broad deny here
    /// breaks every Claude execution, so foreground `Bash` must survive in all
    /// three permission modes, and any `tool_input` we cannot confidently read
    /// as "background" must fall through rather than deny.
    #[tokio::test]
    async fn foreground_bash_is_never_denied_in_any_permission_mode() {
        // `tool_input` shapes that must all be treated as *not* background.
        let inputs = [
            ("absent", serde_json::json!({"command": "ls"})),
            (
                "explicit false",
                serde_json::json!({"command": "ls", "run_in_background": false}),
            ),
            (
                "non-boolean",
                serde_json::json!({"command": "ls", "run_in_background": "true"}),
            ),
            (
                "null",
                serde_json::json!({"command": "ls", "run_in_background": null}),
            ),
            ("empty tool_input", serde_json::json!({})),
        ];

        // bypass/yolo is `auto_approve`; plan and approvals both attach an
        // approval service.
        let clients: [(&str, Arc<ClaudeAgentClient>); 2] = [
            ("bypass", auto_approve_client()),
            ("plan/approvals", approval_client()),
        ];

        for (mode, client) in clients {
            for (label, tool_input) in &inputs {
                let resp = client
                    .on_hook_callback(
                        DENY_BACKGROUND_BASH_CALLBACK_ID.to_string(),
                        bash_hook_input(tool_input.clone()),
                        None,
                    )
                    .await
                    .expect("callback ok");
                assert_ne!(
                    permission_decision(&resp),
                    Some("deny"),
                    "{mode} mode must not deny foreground Bash ({label}); got {resp}"
                );
            }
        }

        // A completely malformed hook input (no `tool_input` at all, wrong
        // type) must also fall through to allow rather than deny.
        let client = auto_approve_client();
        for malformed in [
            serde_json::json!({}),
            serde_json::json!({"tool_input": "not-an-object"}),
            serde_json::json!({"tool_input": null}),
            serde_json::json!([]),
        ] {
            let resp = client
                .on_hook_callback(
                    DENY_BACKGROUND_BASH_CALLBACK_ID.to_string(),
                    malformed.clone(),
                    None,
                )
                .await
                .expect("callback ok");
            assert_eq!(
                permission_decision(&resp),
                Some("allow"),
                "malformed input {malformed} must fall through to allow"
            );
        }
    }

    #[test]
    fn is_background_bash_input_only_matches_explicit_true() {
        assert!(is_background_bash_input(&bash_hook_input(
            serde_json::json!({"run_in_background": true})
        )));
        assert!(!is_background_bash_input(&bash_hook_input(
            serde_json::json!({"run_in_background": false})
        )));
        assert!(!is_background_bash_input(&bash_hook_input(
            serde_json::json!({"run_in_background": "true"})
        )));
        assert!(!is_background_bash_input(&bash_hook_input(
            serde_json::json!({})
        )));
        assert!(!is_background_bash_input(&serde_json::json!({})));
        assert!(!is_background_bash_input(&serde_json::json!(
            "not-an-object"
        )));
    }

    #[tokio::test]
    async fn foreground_bash_defers_to_the_mode_catch_all_when_approvals_are_on() {
        // With an approval service attached, a foreground Bash must return *no*
        // permission decision so plan mode's auto-approve catch-all still
        // allows it. Returning "ask" here would prompt the user for every
        // plan-mode shell command.
        let client = approval_client();
        let resp = client
            .on_hook_callback(
                DENY_BACKGROUND_BASH_CALLBACK_ID.to_string(),
                bash_hook_input(serde_json::json!({"command": "ls"})),
                None,
            )
            .await
            .expect("callback ok");
        assert_eq!(resp, serde_json::json!({}), "expected a no-decision result");
    }

    #[tokio::test]
    async fn unrelated_callback_still_auto_approves() {
        let client = auto_approve_client();
        let resp = client
            .on_hook_callback(
                AUTO_APPROVE_CALLBACK_ID.to_string(),
                serde_json::json!({}),
                None,
            )
            .await
            .expect("callback ok");
        assert_eq!(resp["hookSpecificOutput"]["permissionDecision"], "allow");
    }

    #[test]
    fn unbounded_wait_deny_response_is_a_deny_naming_spawn_poller() {
        let resp = unbounded_wait_deny_response();
        let out = &resp["hookSpecificOutput"];
        assert_eq!(out["hookEventName"], "PreToolUse");
        assert_eq!(out["permissionDecision"], "deny");
        assert_eq!(out["permissionDecisionReason"], UNBOUNDED_WAIT_DENY_REASON);
        // Constitution IX: a denial must name its replacement, or it converts a
        // hang into a stall.
        assert!(
            UNBOUNDED_WAIT_DENY_REASON.contains("spawn_poller"),
            "deny reason must name the supported replacement"
        );
        assert!(
            UNBOUNDED_WAIT_DENY_REASON.contains("stop_command")
                && UNBOUNDED_WAIT_DENY_REASON.contains("timeout_secs"),
            "deny reason must state what the replacement requires"
        );
    }

    #[test]
    fn unbounded_wait_predicate_denies_only_unguarded_wait_loops() {
        // Denied: the shape from the incident, plus its obvious relatives.
        for command in [
            // The reported command: waited ~1h for a line that never arrived.
            "until grep -q \"### restored:\" /tmp/out.log; do sleep 5; done",
            "while true; do sleep 5; done",
            "while :; do sleep 1; done",
            "while ! curl -sf http://localhost:3000; do sleep 2; done",
            "watch -n 5 ls",
            // Bounding a single attempt does not bound the loop: `--timeout` is
            // a flag, not the `timeout` command, so it is not a bound marker.
            r#"until curl --timeout 5 "$url"; do sleep 2; done"#,
            // A loop keyword starting a new line is still command position.
            // `trim_end()` used to eat the newline and let this through.
            "echo start\nuntil grep -q ready /tmp/out.log; do sleep 5; done",
            "set -e\nwhile true; do\n  sleep 5\ndone",
            // A comparison in the *condition* is not a counter bound. The
            // second of these is semantically the incident command.
            r#"while [ $(kubectl get po | grep -c Running) -lt 3 ]; do sleep 5; done"#,
            r#"until [ $(grep -c ready /tmp/out.log) -gt 0 ]; do sleep 5; done"#,
            // `timeout` in a polled *path* is not a deadline.
            "until test -f /tmp/timeout.flag; do sleep 5; done",
        ] {
            assert!(
                is_unbounded_wait_command(&bash_hook_input(
                    serde_json::json!({ "command": command })
                )),
                "expected deny for {command:?}"
            );
        }

        // Allowed: bounded waits, and commands that merely take a long time.
        for command in [
            // Self-bounded (FR-7) — each carries its own exit guarantee.
            "n=0; while [ $n -lt 5 ]; do curl -sf \"$url\" && break; sleep 2; n=$((n+1)); done",
            "timeout 300 bash -c 'until test -f /tmp/ready; do sleep 5; done'",
            "SECONDS=0; while [ $SECONDS -lt 60 ]; do sleep 1; done",
            "for i in $(seq 1 60); do test -f /tmp/ready && break; sleep 1; done",
            // A loop with no wait at all — the `sleep` requirement keeps this out.
            "while read line; do echo \"$line\"; done < input.txt",
            // A rate-limited `while read` loop ends when its input does. These
            // were refused before `read` became a bound marker.
            "cat urls.txt | while read -r u; do curl \"$u\"; sleep 1; done",
            "while read -r line; do echo \"$line\"; sleep 0.5; done < urls.txt",
            // Long, but bounded by its own completion (FR-6).
            "cargo test --workspace",
            "pnpm install --frozen-lockfile",
            "sleep 30",
            // `watch` only counts as the leading token.
            "cargo watch -x test",
            "pnpm run dev --watch",
            // Loop keywords in *prose*, not command position. Without the
            // command-position rule these would all be refused.
            "echo \"retrying until ready\"; sleep 2",
            "sleep 5 && echo 'waiting until the build settles'",
            "git log --until=2026-01-01 && sleep 1",
            // A loop keyword in command position inside a conditional is still
            // a loop, but this one is bounded by a counter.
            "if [ -f x ]; then n=0; while [ $n -lt 3 ]; do sleep 1; n=$((n+1)); done; fi",
        ] {
            assert!(
                !is_unbounded_wait_command(&bash_hook_input(
                    serde_json::json!({ "command": command })
                )),
                "expected allow for {command:?}"
            );
        }
    }

    #[test]
    fn unbounded_wait_predicate_allows_malformed_input() {
        // Ambiguity resolves permissively: `Bash` is the workhorse tool and an
        // over-broad deny would break every Claude execution.
        for malformed in [
            serde_json::json!({}),
            serde_json::json!({"tool_input": "not-an-object"}),
            serde_json::json!({"tool_input": null}),
            serde_json::json!({"tool_input": {}}),
            serde_json::json!({"tool_input": {"command": null}}),
            serde_json::json!({"tool_input": {"command": 42}}),
            serde_json::json!([]),
        ] {
            assert!(
                !is_unbounded_wait_command(&malformed),
                "malformed input must allow: {malformed}"
            );
        }
    }

    #[test]
    fn contains_token_respects_word_boundaries() {
        assert!(contains_token("do sleep 5; done", "sleep"));
        assert!(!contains_token("echo sleeping", "sleep"));
        assert!(!contains_token("nosleep", "sleep"));
        assert!(contains_token("[ $n -lt 5 ]", "-lt"));
        assert!(contains_token("until x", "until"));
        assert!(!contains_token("untilx", "until"));
    }

    #[test]
    fn command_keyword_matching_requires_command_position() {
        // Command position: start, after a separator, or after do/then/else.
        assert!(contains_command_keyword(
            "until foo; do sleep 1; done",
            "until"
        ));
        assert!(contains_command_keyword(
            "x=1; until foo; do sleep 1; done",
            "until"
        ));
        assert!(contains_command_keyword(
            "if y; then while z; do sleep 1; done; fi",
            "while"
        ));
        assert!(contains_command_keyword(
            "foo && while z; do sleep 1; done",
            "while"
        ));

        // Argument or prose position: not a loop.
        assert!(!contains_command_keyword(
            "echo \"wait until ready\"",
            "until"
        ));
        assert!(!contains_command_keyword(
            "git log --until=2026-01-01",
            "until"
        ));
        assert!(!contains_command_keyword("echo awhile", "while"));

        // A newline is a command separator. `trim_end()` here would strip it
        // and silently disable the guard for every multi-line script.
        assert!(contains_command_keyword("echo start\nuntil foo", "until"));
        assert!(contains_command_keyword(
            "echo start\r\n  while foo",
            "while"
        ));

        // `timeout` counts as a bound marker only as a command, not in a path.
        assert!(contains_command_keyword("timeout 300 bash -c x", "timeout"));
        assert!(!contains_command_keyword(
            "test -f /tmp/timeout.flag",
            "timeout"
        ));
    }

    #[tokio::test]
    async fn unbounded_wait_is_denied_even_in_auto_approve() {
        // Same placement rule as the background deny: checked before the
        // auto-approve short-circuit, so it fires in bypass/yolo — the mode the
        // incident occurred in.
        let clients: [(&str, Arc<ClaudeAgentClient>); 2] = [
            ("bypass", auto_approve_client()),
            ("plan/approvals", approval_client()),
        ];

        for (mode, client) in clients {
            let resp = client
                .on_hook_callback(
                    DENY_BACKGROUND_BASH_CALLBACK_ID.to_string(),
                    bash_hook_input(serde_json::json!({
                        "command": r#"until grep -q ready /tmp/out.log; do sleep 5; done"#
                    })),
                    None,
                )
                .await
                .expect("callback ok");
            assert_eq!(
                permission_decision(&resp),
                Some("deny"),
                "{mode} mode must deny an unbounded wait; got {resp}"
            );
        }
    }

    #[tokio::test]
    async fn background_message_wins_when_a_call_is_both() {
        // A background spawn that is also a wait loop gets the background
        // message: it is the more specific diagnosis of what went wrong.
        let client = auto_approve_client();
        let resp = client
            .on_hook_callback(
                DENY_BACKGROUND_BASH_CALLBACK_ID.to_string(),
                bash_hook_input(serde_json::json!({
                    "command": "while true; do sleep 5; done",
                    "run_in_background": true
                })),
                None,
            )
            .await
            .expect("callback ok");
        assert_eq!(
            resp["hookSpecificOutput"]["permissionDecisionReason"],
            BACKGROUND_BASH_DENY_REASON
        );
    }
}
