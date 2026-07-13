//! Block Kit view builders for the create-issue modal.

use serde_json::{Value, json};
use uuid::Uuid;

use super::types::CREATE_ISSUE_MODAL_CALLBACK_ID;

/// Slack's hard cap on `static_select` options.
pub const MAX_PROJECT_OPTIONS: usize = 100;
/// Slack caps `plain_text` option labels at 75 chars.
const OPTION_LABEL_MAX_CHARS: usize = 75;

// Input identifiers. Slack `views.update` **preserves the current input state**
// for any block whose `block_id`+`action_id` are unchanged and ignores the new
// `initial_value` — so to make the AI summary actually replace the mechanical
// prefill on update, the AI-rendered view must use DISTINCT title/description
// ids. `view_submission` accepts either set. The project select keeps stable
// ids on purpose, so the user's project choice survives the update.
pub const TITLE_BLOCK_ID: &str = "title";
pub const TITLE_ACTION_ID: &str = "title_input";
pub const DESCRIPTION_BLOCK_ID: &str = "description";
pub const DESCRIPTION_ACTION_ID: &str = "description_input";
pub const TITLE_BLOCK_ID_AI: &str = "title_ai";
pub const TITLE_ACTION_ID_AI: &str = "title_input_ai";
pub const DESCRIPTION_BLOCK_ID_AI: &str = "description_ai";
pub const DESCRIPTION_ACTION_ID_AI: &str = "description_input_ai";

pub struct ProjectOption {
    pub id: Uuid,
    pub name: String,
}

pub struct CreateIssueModal {
    pub view: Value,
    /// Projects dropped by the 100-option cap; callers log this so the cap
    /// is never silent.
    pub truncated_projects: usize,
}

fn option_label(name: &str) -> String {
    if name.chars().count() <= OPTION_LABEL_MAX_CHARS {
        return name.to_string();
    }
    let mut label: String = name.chars().take(OPTION_LABEL_MAX_CHARS - 1).collect();
    label.push('…');
    label
}

/// The create-issue modal (contract §2): project select + title +
/// description, prefilled from the message. `private_metadata` is the
/// serialized [`super::types::ModalMetadata`].
///
/// When `hint` is `Some`, a leading context block is shown — used for the
/// "✨ Summarizing thread…" notice while the AI summary is generated (FR-8).
/// The follow-up `views.update` re-renders with `hint = None`.
///
/// `ai_variant = true` renders the title/description inputs with the AI-set
/// ids so a `views.update` actually shows the new values (see the id consts
/// above); the initial open and the failure-revert both use `false`.
pub fn build_create_issue_modal(
    projects: &[ProjectOption],
    title_prefill: &str,
    description_prefill: &str,
    private_metadata: &str,
    hint: Option<&str>,
    ai_variant: bool,
) -> CreateIssueModal {
    let (title_block, title_action, description_block, description_action) = if ai_variant {
        (
            TITLE_BLOCK_ID_AI,
            TITLE_ACTION_ID_AI,
            DESCRIPTION_BLOCK_ID_AI,
            DESCRIPTION_ACTION_ID_AI,
        )
    } else {
        (
            TITLE_BLOCK_ID,
            TITLE_ACTION_ID,
            DESCRIPTION_BLOCK_ID,
            DESCRIPTION_ACTION_ID,
        )
    };

    let truncated_projects = projects.len().saturating_sub(MAX_PROJECT_OPTIONS);
    let options: Vec<Value> = projects
        .iter()
        .take(MAX_PROJECT_OPTIONS)
        .map(|p| {
            json!({
                "text": {"type": "plain_text", "text": option_label(&p.name)},
                "value": p.id.to_string(),
            })
        })
        .collect();

    let mut title_input = json!({
        "type": "plain_text_input",
        "action_id": title_action,
        "max_length": 500,
    });
    if !title_prefill.is_empty() {
        title_input["initial_value"] = json!(title_prefill);
    }

    let mut description_input = json!({
        "type": "plain_text_input",
        "action_id": description_action,
        "multiline": true,
    });
    if !description_prefill.is_empty() {
        description_input["initial_value"] = json!(description_prefill);
    }

    let mut blocks: Vec<Value> = Vec::new();
    if let Some(hint) = hint.filter(|h| !h.is_empty()) {
        blocks.push(json!({
            "type": "context",
            "elements": [{"type": "mrkdwn", "text": hint}],
        }));
    }
    blocks.push(json!({
        "type": "input",
        "block_id": "project",
        "label": {"type": "plain_text", "text": "Project"},
        "element": {
            "type": "static_select",
            "action_id": "project_select",
            "placeholder": {"type": "plain_text", "text": "Select a project"},
            "options": options,
        },
    }));
    blocks.push(json!({
        "type": "input",
        "block_id": title_block,
        "label": {"type": "plain_text", "text": "Title"},
        "element": title_input,
    }));
    blocks.push(json!({
        "type": "input",
        "block_id": description_block,
        "optional": true,
        "label": {"type": "plain_text", "text": "Description"},
        "element": description_input,
    }));

    let view = json!({
        "type": "modal",
        "callback_id": CREATE_ISSUE_MODAL_CALLBACK_ID,
        "private_metadata": private_metadata,
        "title": {"type": "plain_text", "text": "Create issue"},
        "submit": {"type": "plain_text", "text": "Create"},
        "close": {"type": "plain_text", "text": "Cancel"},
        "blocks": blocks,
    });

    CreateIssueModal {
        view,
        truncated_projects,
    }
}

/// Informational modal for the "told, not a dead control" cases (FR-7):
/// integration disabled, no projects, etc.
pub fn build_info_modal(text: &str) -> Value {
    json!({
        "type": "modal",
        "title": {"type": "plain_text", "text": "Vibe Kanban"},
        "close": {"type": "plain_text", "text": "Close"},
        "blocks": [
            {
                "type": "section",
                "text": {"type": "mrkdwn", "text": text},
            },
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projects(n: usize) -> Vec<ProjectOption> {
        (0..n)
            .map(|i| ProjectOption {
                id: Uuid::from_u128(i as u128),
                name: format!("Project {i}"),
            })
            .collect()
    }

    #[test]
    fn modal_has_expected_blocks_and_callback() {
        let modal =
            build_create_issue_modal(&projects(2), "Title", "Desc", "{\"a\":1}", None, false);
        assert_eq!(modal.truncated_projects, 0);
        assert_eq!(modal.view["callback_id"], CREATE_ISSUE_MODAL_CALLBACK_ID);
        assert_eq!(modal.view["private_metadata"], "{\"a\":1}");
        let blocks = modal.view["blocks"].as_array().unwrap();
        let block_ids: Vec<&str> = blocks
            .iter()
            .map(|b| b["block_id"].as_str().unwrap())
            .collect();
        assert_eq!(block_ids, vec!["project", "title", "description"]);
        assert_eq!(
            blocks[1]["element"]["initial_value"].as_str(),
            Some("Title")
        );
        assert_eq!(blocks[2]["optional"], true);
    }

    #[test]
    fn ai_variant_uses_distinct_input_ids() {
        // The AI-updated view must use fresh block/action ids so Slack renders
        // the new values on views.update (input-state preservation gotcha).
        let modal = build_create_issue_modal(&projects(1), "AiTitle", "AiDesc", "{}", None, true);
        let blocks = modal.view["blocks"].as_array().unwrap();
        assert_eq!(blocks[0]["block_id"], "project");
        assert_eq!(blocks[1]["block_id"], TITLE_BLOCK_ID_AI);
        assert_eq!(blocks[1]["element"]["action_id"], TITLE_ACTION_ID_AI);
        assert_eq!(blocks[2]["block_id"], DESCRIPTION_BLOCK_ID_AI);
        assert_eq!(blocks[2]["element"]["action_id"], DESCRIPTION_ACTION_ID_AI);
        assert_eq!(blocks[1]["element"]["initial_value"], "AiTitle");
    }

    #[test]
    fn empty_prefills_omit_initial_value() {
        // Slack rejects initial_value: "" — the key must be absent instead.
        let modal = build_create_issue_modal(&projects(1), "", "", "{}", None, false);
        let blocks = modal.view["blocks"].as_array().unwrap();
        assert!(blocks[1]["element"].get("initial_value").is_none());
        assert!(blocks[2]["element"].get("initial_value").is_none());
    }

    #[test]
    fn hint_adds_leading_context_block() {
        let with =
            build_create_issue_modal(&projects(1), "t", "d", "{}", Some("✨ Summarizing…"), false);
        let blocks = with.view["blocks"].as_array().unwrap();
        assert_eq!(blocks[0]["type"], "context");
        // Inputs shift down by one; project is now at index 1.
        assert_eq!(blocks[1]["block_id"], "project");
        assert_eq!(blocks[3]["block_id"], "description");

        // Empty hint is treated as no hint.
        let without = build_create_issue_modal(&projects(1), "t", "d", "{}", Some(""), false);
        assert_eq!(without.view["blocks"][0]["block_id"], "project");
    }

    #[test]
    fn caps_projects_at_100_and_reports_truncation() {
        let modal = build_create_issue_modal(&projects(130), "t", "d", "{}", None, false);
        let options = modal.view["blocks"][0]["element"]["options"]
            .as_array()
            .unwrap();
        assert_eq!(options.len(), MAX_PROJECT_OPTIONS);
        assert_eq!(modal.truncated_projects, 30);
    }

    #[test]
    fn long_project_names_fit_option_label_cap() {
        let long_name = "p".repeat(200);
        let modal = build_create_issue_modal(
            &[ProjectOption {
                id: Uuid::nil(),
                name: long_name,
            }],
            "t",
            "d",
            "{}",
            None,
            false,
        );
        let label = modal.view["blocks"][0]["element"]["options"][0]["text"]["text"]
            .as_str()
            .unwrap();
        assert_eq!(label.chars().count(), 75);
    }
}
