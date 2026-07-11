//! Block Kit view builders for the create-issue modal.

use serde_json::{Value, json};
use uuid::Uuid;

use super::types::CREATE_ISSUE_MODAL_CALLBACK_ID;

/// Slack's hard cap on `static_select` options.
pub const MAX_PROJECT_OPTIONS: usize = 100;
/// Slack caps `plain_text` option labels at 75 chars.
const OPTION_LABEL_MAX_CHARS: usize = 75;

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
pub fn build_create_issue_modal(
    projects: &[ProjectOption],
    title_prefill: &str,
    description_prefill: &str,
    private_metadata: &str,
) -> CreateIssueModal {
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
        "action_id": "title_input",
        "max_length": 500,
    });
    if !title_prefill.is_empty() {
        title_input["initial_value"] = json!(title_prefill);
    }

    let mut description_input = json!({
        "type": "plain_text_input",
        "action_id": "description_input",
        "multiline": true,
    });
    if !description_prefill.is_empty() {
        description_input["initial_value"] = json!(description_prefill);
    }

    let view = json!({
        "type": "modal",
        "callback_id": CREATE_ISSUE_MODAL_CALLBACK_ID,
        "private_metadata": private_metadata,
        "title": {"type": "plain_text", "text": "Create issue"},
        "submit": {"type": "plain_text", "text": "Create"},
        "close": {"type": "plain_text", "text": "Cancel"},
        "blocks": [
            {
                "type": "input",
                "block_id": "project",
                "label": {"type": "plain_text", "text": "Project"},
                "element": {
                    "type": "static_select",
                    "action_id": "project_select",
                    "placeholder": {"type": "plain_text", "text": "Select a project"},
                    "options": options,
                },
            },
            {
                "type": "input",
                "block_id": "title",
                "label": {"type": "plain_text", "text": "Title"},
                "element": title_input,
            },
            {
                "type": "input",
                "block_id": "description",
                "optional": true,
                "label": {"type": "plain_text", "text": "Description"},
                "element": description_input,
            },
        ],
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
        let modal = build_create_issue_modal(&projects(2), "Title", "Desc", "{\"a\":1}");
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
    fn empty_prefills_omit_initial_value() {
        // Slack rejects initial_value: "" — the key must be absent instead.
        let modal = build_create_issue_modal(&projects(1), "", "", "{}");
        let blocks = modal.view["blocks"].as_array().unwrap();
        assert!(blocks[1]["element"].get("initial_value").is_none());
        assert!(blocks[2]["element"].get("initial_value").is_none());
    }

    #[test]
    fn caps_projects_at_100_and_reports_truncation() {
        let modal = build_create_issue_modal(&projects(130), "t", "d", "{}");
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
        );
        let label = modal.view["blocks"][0]["element"]["options"][0]["text"]["text"]
            .as_str()
            .unwrap();
        assert_eq!(label.chars().count(), 75);
    }
}
