//! `list_pipelines` MCP tool, plus the pipeline shape `create_issue` uses to
//! attach a pipeline to a new issue. Block composition itself lives in
//! `api_types::pipeline_block`, shared with auto error remediation, so every
//! backend caller produces byte-for-byte the same `## Pipeline` block the New
//! Issue UI does.

use api_types::pipeline_block::{BlockPipeline, BlockStage};
use rmcp::{ErrorData, model::CallToolResult, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};

use super::McpServer;

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct McpPipelineStep {
    #[schemars(description = "Stable stage id, e.g. 'spec'")]
    pub id: String,
    #[schemars(description = "Display label for the stage")]
    pub label: String,
    #[schemars(
        description = "The stage's instruction text; rendered as a numbered line when the stage is enabled"
    )]
    pub prompt_fragment: String,
    #[schemars(
        description = "Whether this stage is ticked by default in the UI (used as the default enabled set when a `create_issue` call omits `pipeline_stage_ids`)"
    )]
    pub default_enabled: bool,
    #[schemars(description = "Whether this stage is marked resource-intensive")]
    pub heavy: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct McpPipeline {
    #[schemars(description = "Stable pipeline id, e.g. 'basic'")]
    pub id: String,
    #[schemars(description = "Display name")]
    pub name: String,
    #[schemars(description = "Optional one-line description")]
    pub description: Option<String>,
    #[schemars(description = "Ordered stages; this order is authoritative for the composed block")]
    pub stages: Vec<McpPipelineStep>,
}

/// The subset of `GET /api/pipelines/new-issue-defaults` that `create_issue`
/// needs: the resolved, already-filtered default pipeline ids.
#[derive(Debug, Deserialize)]
pub(crate) struct NewIssuePipelineDefaults {
    pub pipeline_ids: Vec<String>,
}

/// Pick `ids` out of `catalog`, in `ids` order. An explicit (caller-named)
/// id that is missing is an error; a defaulted one is skipped, because a
/// default naming a since-deleted pipeline must never block issue creation.
pub(crate) fn select_pipelines(
    catalog: &[McpPipeline],
    ids: &[String],
    explicit: bool,
) -> Result<Vec<McpPipeline>, String> {
    let mut selected = Vec::with_capacity(ids.len());
    for id in ids {
        match catalog.iter().find(|p| &p.id == id) {
            Some(p) => selected.push(p.clone()),
            None if explicit => {
                return Err(format!(
                    "Unknown pipeline '{id}'. Use list_pipelines to see available pipelines."
                ));
            }
            None => tracing::warn!("Skipping missing default pipeline '{id}'"),
        }
    }
    Ok(selected)
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct ListPipelinesResponse {
    pipelines: Vec<McpPipeline>,
    count: usize,
}

impl From<&McpPipeline> for BlockPipeline {
    fn from(pipeline: &McpPipeline) -> Self {
        BlockPipeline {
            name: pipeline.name.clone(),
            stages: pipeline
                .stages
                .iter()
                .map(|s| BlockStage {
                    id: s.id.clone(),
                    prompt_fragment: s.prompt_fragment.clone(),
                })
                .collect(),
        }
    }
}

#[tool_router(router = pipelines_tools_router, vis = "pub")]
impl McpServer {
    #[tool(
        description = "List the available task pipelines (from ~/.vibe-kanban/pipelines/*.toml). Pass a pipeline's `id` and its stage `id`s to `create_issue`'s `pipeline_ids`/`pipeline_stage_ids` to attach a pipeline block to a new issue. When `pipeline_ids` is omitted, `create_issue` attaches the configured new-issue default pipelines (Settings → Pipelines; project, then organization, then machine); pass `pipeline_ids: []` for none."
    )]
    async fn list_pipelines(&self) -> Result<CallToolResult, ErrorData> {
        let url = self.url("/api/pipelines");
        let pipelines: Vec<McpPipeline> = match self.send_json(self.client.get(&url)).await {
            Ok(p) => p,
            Err(e) => return Ok(Self::tool_error(e)),
        };

        McpServer::success(&ListPipelinesResponse {
            count: pipelines.len(),
            pipelines,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pipeline(id: &str) -> McpPipeline {
        McpPipeline {
            id: id.to_string(),
            name: id.to_string(),
            description: None,
            stages: Vec::new(),
        }
    }

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn selects_in_requested_order() {
        let catalog = vec![pipeline("basic"), pipeline("wikillm"), pipeline("speckit")];
        let selected = select_pipelines(&catalog, &ids(&["speckit", "wikillm"]), true).unwrap();
        let got: Vec<&str> = selected.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(got, vec!["speckit", "wikillm"]);
    }

    #[test]
    fn explicit_missing_pipeline_is_an_error() {
        let catalog = vec![pipeline("basic")];
        let err = select_pipelines(&catalog, &ids(&["gone"]), true).unwrap_err();
        assert!(err.contains("Unknown pipeline 'gone'"));
    }

    #[test]
    fn defaulted_missing_pipeline_is_skipped() {
        let catalog = vec![pipeline("basic")];
        let selected = select_pipelines(&catalog, &ids(&["gone", "basic"]), false).unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id, "basic");
    }
}
