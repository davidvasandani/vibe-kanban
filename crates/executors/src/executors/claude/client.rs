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
            ClaudeJson,
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
pub const BACKGROUND_BASH_DENY_REASON: &str = "Background processes are not supported inside a Vibe Kanban turn: this turn's process group is terminated when the turn ends, so anything started with run_in_background is reaped with it and any output you meant to poll is silently lost. Run the command in the foreground instead. If it genuinely needs to outlive this turn, use the `spawn_poller` MCP tool, which runs it in its own process group that survives the turn and is visible in the workspace UI. Either way, keep working in this turn instead of waiting on a background process.";

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
}

impl ClaudeAgentClient {
    /// Create a new client with optional approval service
    pub fn new(
        log_writer: LogWriter,
        approvals: Option<Arc<dyn ExecutorApprovalService>>,
        repo_context: RepoContext,
        commit_reminder_prompt: String,
        cancel: CancellationToken,
    ) -> Arc<Self> {
        let auto_approve = approvals.is_none();
        Arc::new(Self {
            log_writer,
            approvals,
            auto_approve,
            repo_context,
            commit_reminder_prompt,
            cancel,
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
        if callback_id == DENY_BACKGROUND_BASH_CALLBACK_ID && is_background_bash_input(&input) {
            return Ok(background_bash_deny_response());
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
}
