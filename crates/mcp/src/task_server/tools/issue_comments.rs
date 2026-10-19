//! Issue comment tools. Creating a reply goes through the backend's shared
//! delivery path (`POST /api/issue-comments`), the same one the issue panel
//! uses, so the result reports the persisted delivery outcome rather than
//! "saved". Saving a comment is never reported as delivery to an agent.

use api_types::IssueComment;
use db::models::issue_comment_delivery::IssueCommentDelivery;
use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, schemars, tool,
    tool_router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::McpServer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum McpCommentIntent {
    /// Saved on the issue only. Never sent to any agent, never resumes work.
    Note,
    /// Delivered to the agent session linked to the issue: resumes it if it
    /// is waiting for input, or queues it for the end of its current turn.
    ReplyToAgent,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct McpCreateIssueCommentRequest {
    #[schemars(
        description = "Issue to comment on. Accepts the issue UUID or its simple ID (e.g. 'VAS-64')."
    )]
    issue_id: String,
    #[schemars(description = "Comment text (markdown).")]
    message: String,
    #[schemars(
        description = "Required. 'note': saved on the issue only; no agent receives it and no work restarts. 'reply_to_agent': also delivered to the agent session linked to the issue (resumes it if it is waiting for input, or queues it for the end of its running turn). Choose explicitly; intent is never inferred from the text."
    )]
    intent: McpCommentIntent,
    #[schemars(
        description = "Optional session to deliver a reply to. Needed when several sessions are linked (delivery state 'needs_target' lists them). An explicit choice may also resume a session whose last turn failed or was stopped."
    )]
    target_session_id: Option<Uuid>,
    #[schemars(description = "Optional parent comment ID to reply in a thread.")]
    parent_id: Option<Uuid>,
    #[schemars(
        description = "Optional idempotency key. Retrying with the same comment_id never creates a second comment or delivery; omit to have one generated (it is returned)."
    )]
    comment_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
struct CreateIssueCommentPayload {
    issue_id: Uuid,
    message: String,
    intent: McpCommentIntent,
    parent_id: Option<Uuid>,
    target_session_id: Option<Uuid>,
    comment_id: Option<Uuid>,
    origin_session_id: Option<Uuid>,
    origin_workspace_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct CreateIssueCommentResult {
    comment: IssueComment,
    issue_url: String,
    delivery: Option<IssueCommentDelivery>,
}

/// The delivery outcome as an agent should read it.
#[derive(Debug, Serialize, schemars::JsonSchema)]
struct DeliverySummary {
    #[schemars(
        description = "not_requested (note) | pending (queued for the session's turn boundary or a scheduled retry) | delivered (a turn carrying the comment started) | needs_target (choose target_session_id) | no_target (see reason) | failed (see error; retry with deliver_issue_comment) | blocked (loop guard) | cancelled (comment deleted)"
    )]
    state: String,
    reason: Option<String>,
    error: Option<String>,
    workspace_id: Option<String>,
    session_id: Option<String>,
    #[schemars(description = "Execution started by the delivery (when delivered)")]
    execution_id: Option<String>,
    #[schemars(description = "What happened, in one sentence")]
    message: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct McpCreateIssueCommentResponse {
    comment_id: String,
    issue_url: String,
    saved_on_issue: bool,
    delivery: DeliverySummary,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct McpListIssueCommentsRequest {
    #[schemars(description = "Issue UUID or simple ID (e.g. 'VAS-64').")]
    issue_id: String,
}

#[derive(Debug, Deserialize)]
struct CommentWithDelivery {
    comment: IssueComment,
    author_name: Option<String>,
    delivery: Option<IssueCommentDelivery>,
    edited_after_delivery: bool,
}

#[derive(Debug, Deserialize)]
struct IssueCommentsWithDelivery {
    issue_url: Option<String>,
    comments: Vec<CommentWithDelivery>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct CommentSummary {
    comment_id: String,
    author_name: Option<String>,
    message: String,
    parent_id: Option<String>,
    created_at: String,
    updated_at: String,
    #[schemars(description = "Delivery to an agent, if one was requested")]
    delivery: Option<DeliverySummary>,
    #[schemars(
        description = "True when the comment was edited after delivery: the agent received the original text, which is not replayed."
    )]
    edited_after_delivery: bool,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct McpListIssueCommentsResponse {
    issue_id: String,
    issue_url: Option<String>,
    count: usize,
    comments: Vec<CommentSummary>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct McpDeliverIssueCommentRequest {
    #[schemars(description = "Issue UUID or simple ID the comment belongs to.")]
    issue_id: String,
    #[schemars(description = "Existing comment to deliver to an agent.")]
    comment_id: Uuid,
    #[schemars(
        description = "Optional session to deliver to (required when the delivery needs a target)."
    )]
    target_session_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
struct RequestDeliveryPayload {
    comment_id: Uuid,
    issue_id: Uuid,
    target_session_id: Option<Uuid>,
    source: &'static str,
    origin_session_id: Option<Uuid>,
    origin_workspace_id: Option<Uuid>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct McpDeliverIssueCommentResponse {
    comment_id: String,
    delivery: DeliverySummary,
}

fn wire_name<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn summarize(delivery: Option<&IssueCommentDelivery>) -> DeliverySummary {
    let Some(delivery) = delivery else {
        return DeliverySummary {
            state: "not_requested".into(),
            reason: None,
            error: None,
            workspace_id: None,
            session_id: None,
            execution_id: None,
            message: "Saved on the issue as a note; not sent to any agent.".into(),
        };
    };
    let state = wire_name(&delivery.state);
    let reason = delivery.reason.as_ref().map(wire_name);
    let message = match state.as_str() {
        "delivered" => format!(
            "Delivered to session {}; execution {} started with the comment.",
            delivery.session_id.map(|id| id.to_string()).unwrap_or_default(),
            delivery
                .execution_process_id
                .map(|id| id.to_string())
                .unwrap_or_default()
        ),
        "pending" => format!(
            "Queued for session {} ({}); it is delivered at the end of the agent's current turn or at the scheduled retry.",
            delivery.session_id.map(|id| id.to_string()).unwrap_or_default(),
            reason.clone().unwrap_or_else(|| "waiting".into())
        ),
        "needs_target" => "Several linked sessions are eligible; call deliver_issue_comment with target_session_id.".into(),
        "no_target" => format!(
            "Saved on the issue, but no session can receive it ({}).",
            reason.clone().unwrap_or_default()
        ),
        "failed" => "Saved on the issue, but delivery failed; call deliver_issue_comment to retry.".into(),
        "blocked" => format!(
            "Saved on the issue; delivery was refused ({}).",
            reason.clone().unwrap_or_default()
        ),
        "cancelled" => "The comment was deleted before delivery.".into(),
        _ => "Delivery is being dispatched.".into(),
    };
    DeliverySummary {
        state,
        reason,
        error: delivery.error.clone(),
        workspace_id: delivery.workspace_id.map(|id| id.to_string()),
        session_id: delivery.session_id.map(|id| id.to_string()),
        execution_id: delivery.execution_process_id.map(|id| id.to_string()),
        message,
    }
}

#[tool_router(router = issue_comments_tools_router, vis = "pub")]
impl McpServer {
    #[tool(
        description = "Post a comment on an issue. `intent` is required: 'note' only saves it on the issue; 'reply_to_agent' also delivers it to the agent session linked to the issue (resuming it if it is waiting for input, or queueing it for the end of its running turn) and returns the delivery outcome with the started execution id. A saved comment is not a delivery: read `delivery.state`."
    )]
    async fn create_issue_comment(
        &self,
        Parameters(request): Parameters<McpCreateIssueCommentRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        if request.message.trim().is_empty() {
            return Self::err("message must not be empty", None);
        }
        let issue_id = match self.resolve_issue_id(&request.issue_id).await {
            Ok(id) => id,
            Err(e) => return Ok(Self::tool_error(e)),
        };
        // The idempotency key is fixed here, so a retried call is safe even
        // when the first response never arrived.
        let comment_id = request.comment_id.unwrap_or_else(Uuid::new_v4);
        let payload = CreateIssueCommentPayload {
            issue_id,
            message: request.message,
            intent: request.intent,
            parent_id: request.parent_id,
            target_session_id: request.target_session_id,
            comment_id: Some(comment_id),
            origin_session_id: self.orchestrator_session_id(),
            origin_workspace_id: self.scoped_workspace_id(),
        };
        let url = self.url("/api/issue-comments");
        let result: CreateIssueCommentResult =
            match self.send_json(self.client.post(&url).json(&payload)).await {
                Ok(result) => result,
                Err(e) => return Ok(Self::tool_error(e)),
            };
        Self::success(&McpCreateIssueCommentResponse {
            comment_id: result.comment.id.to_string(),
            issue_url: result.issue_url,
            saved_on_issue: true,
            delivery: summarize(result.delivery.as_ref()),
        })
    }

    #[tool(
        description = "List an issue's comments, oldest first, with author, timestamps and each comment's delivery state to agent sessions."
    )]
    async fn list_issue_comments(
        &self,
        Parameters(McpListIssueCommentsRequest { issue_id }): Parameters<
            McpListIssueCommentsRequest,
        >,
    ) -> Result<CallToolResult, ErrorData> {
        let issue_id = match self.resolve_issue_id(&issue_id).await {
            Ok(id) => id,
            Err(e) => return Ok(Self::tool_error(e)),
        };
        let url = self.url(&format!("/api/issues/{issue_id}/comments"));
        let result: IssueCommentsWithDelivery = match self.send_json(self.client.get(&url)).await {
            Ok(result) => result,
            Err(e) => return Ok(Self::tool_error(e)),
        };
        let comments: Vec<CommentSummary> = result
            .comments
            .into_iter()
            .map(|entry| CommentSummary {
                comment_id: entry.comment.id.to_string(),
                author_name: entry.author_name,
                message: entry.comment.message,
                parent_id: entry.comment.parent_id.map(|id| id.to_string()),
                created_at: entry.comment.created_at.to_rfc3339(),
                updated_at: entry.comment.updated_at.to_rfc3339(),
                delivery: entry.delivery.as_ref().map(|d| summarize(Some(d))),
                edited_after_delivery: entry.edited_after_delivery,
            })
            .collect();
        Self::success(&McpListIssueCommentsResponse {
            issue_id: issue_id.to_string(),
            issue_url: result.issue_url,
            count: comments.len(),
            comments,
        })
    }

    #[tool(
        description = "Deliver an existing issue comment to the linked agent session: retries a failed delivery or chooses the target when several sessions are eligible (`target_session_id`). Idempotent: a comment is delivered at most once."
    )]
    async fn deliver_issue_comment(
        &self,
        Parameters(request): Parameters<McpDeliverIssueCommentRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let issue_id = match self.resolve_issue_id(&request.issue_id).await {
            Ok(id) => id,
            Err(e) => return Ok(Self::tool_error(e)),
        };
        let payload = RequestDeliveryPayload {
            comment_id: request.comment_id,
            issue_id,
            target_session_id: request.target_session_id,
            source: "mcp",
            origin_session_id: self.orchestrator_session_id(),
            origin_workspace_id: self.scoped_workspace_id(),
        };
        let url = self.url("/api/issue-comment-deliveries");
        let delivery: IssueCommentDelivery =
            match self.send_json(self.client.post(&url).json(&payload)).await {
                Ok(delivery) => delivery,
                Err(e) => return Ok(Self::tool_error(e)),
            };
        Self::success(&McpDeliverIssueCommentResponse {
            comment_id: request.comment_id.to_string(),
            delivery: summarize(Some(&delivery)),
        })
    }
}

#[cfg(test)]
mod tests {
    use db::models::issue_comment_delivery::{
        IssueCommentDeliveryReason, IssueCommentDeliverySource, IssueCommentDeliveryState,
    };

    use super::*;

    #[test]
    fn intent_is_required_and_uses_snake_case() {
        let schema = schemars::schema_for!(McpCreateIssueCommentRequest);
        let value = serde_json::to_value(&schema).unwrap();
        let required: Vec<&str> = value["required"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(required.contains(&"intent"));
        assert!(required.contains(&"message"));
        let missing = serde_json::from_value::<McpCreateIssueCommentRequest>(
            serde_json::json!({ "issue_id": "VAS-1", "message": "hi" }),
        );
        assert!(missing.is_err(), "intent has no default");
        let reply: McpCreateIssueCommentRequest = serde_json::from_value(serde_json::json!({
            "issue_id": "VAS-1", "message": "hi", "intent": "reply_to_agent"
        }))
        .unwrap();
        assert_eq!(reply.intent, McpCommentIntent::ReplyToAgent);
    }

    fn delivery(state: IssueCommentDeliveryState) -> IssueCommentDelivery {
        // Built from the wire shape the backend returns.
        serde_json::from_value(serde_json::json!({
            "id": Uuid::new_v4(),
            "comment_id": Uuid::new_v4(),
            "issue_id": Uuid::new_v4(),
            "workspace_id": Uuid::new_v4(),
            "session_id": Uuid::new_v4(),
            "execution_process_id": Uuid::new_v4(),
            "state": state,
            "reason": IssueCommentDeliveryReason::AgentRunning,
            "error": null,
            "source": IssueCommentDeliverySource::Mcp,
            "origin_session_id": null,
            "hop_count": 0,
            "comment_author_id": null,
            "comment_created_at": "2026-10-19T04:45:00Z",
            "delivered_message": null,
            "delivered_comment_updated_at": null,
            "attempt_count": 0,
            "next_attempt_at": null,
            "allow_stopped_target": false,
            "created_at": "2026-10-19T04:45:00Z",
            "updated_at": "2026-10-19T04:45:00Z",
            "delivered_at": null
        }))
        .unwrap()
    }

    #[test]
    fn summaries_never_present_saving_as_delivery() {
        let note = summarize(None);
        assert_eq!(note.state, "not_requested");
        assert!(note.message.contains("not sent to any agent"));

        let delivered = delivery(IssueCommentDeliveryState::Delivered);
        let summary = summarize(Some(&delivered));
        assert_eq!(summary.state, "delivered");
        assert_eq!(
            summary.execution_id,
            delivered.execution_process_id.map(|id| id.to_string())
        );

        let pending = summarize(Some(&delivery(IssueCommentDeliveryState::Pending)));
        assert_eq!(pending.reason.as_deref(), Some("agent_running"));
        assert!(pending.message.starts_with("Queued for session"));

        let failed = summarize(Some(&delivery(IssueCommentDeliveryState::Failed)));
        assert!(failed.message.contains("deliver_issue_comment"));
    }
}
