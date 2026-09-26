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
        description = "List the available task pipelines (from ~/.vibe-kanban/pipelines/*.toml). Pass a pipeline's `id` and its stage `id`s to `create_issue`'s `pipeline_ids`/`pipeline_stage_ids` to attach a pipeline block to a new issue."
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
