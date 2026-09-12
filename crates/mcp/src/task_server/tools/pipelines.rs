//! `list_pipelines` MCP tool, plus the block-composition helpers `create_issue`
//! uses to attach a pipeline to a new issue. This mirrors (in Rust) the
//! `composePipelineBlock`/`canonicalStageOrder` logic in
//! `packages/web-core/src/shared/lib/pipeline/taskPipeline.ts`, so an MCP
//! client produces byte-for-byte the same `## Pipeline` block the New Issue UI
//! does. Keep the two in sync if the block format changes.

use std::collections::{HashMap, HashSet};

use rmcp::{ErrorData, model::CallToolResult, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};

use super::McpServer;

const PIPELINE_START: &str = "<!-- vk:pipeline:start -->";
const PIPELINE_END: &str = "<!-- vk:pipeline:end -->";
const ORDER_INSTRUCTION: &str = "Execute these stages in the order listed. Do not add, skip, or reorder stages. As you begin each numbered stage below, output a single line exactly `VK-PIPELINE-STAGE: N` (N = the number of the stage you are starting) so pipeline progress can be tracked.";

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

/// Compose the executor-pin line embedded at the top of a pipeline block, or
/// an empty string when no agent is pinned. Mirrors `composeExecutorLine`.
pub(crate) fn compose_executor_line(executor: Option<&str>) -> String {
    let trimmed = executor.map(str::trim).unwrap_or("");
    if trimmed.is_empty() {
        return String::new();
    }
    format!(
        "- Run this task with the **{trimmed}** execution agent: pass `executor: \"{trimmed}\"` when starting the workspace."
    )
}

/// Merge the full stage sequences of the given pipelines into one canonical
/// order via a stable topological sort (Kahn's algorithm), so a stage shared
/// by two pipelines appears once and each pipeline's declared relative order
/// is respected. Ties break by first-seen index across pipelines in
/// selection order. Mirrors `canonicalStageOrder`.
pub(crate) fn canonical_stage_order(pipelines: &[McpPipeline]) -> Vec<McpPipelineStep> {
    let mut by_id: HashMap<String, McpPipelineStep> = HashMap::new();
    let mut first_seen: HashMap<String, usize> = HashMap::new();
    let mut seen_idx = 0usize;
    for p in pipelines {
        for s in &p.stages {
            by_id.entry(s.id.clone()).or_insert_with(|| s.clone());
            first_seen.entry(s.id.clone()).or_insert_with(|| {
                let idx = seen_idx;
                seen_idx += 1;
                idx
            });
        }
    }

    let mut indegree: HashMap<String, usize> = by_id.keys().map(|id| (id.clone(), 0)).collect();
    let mut adjacency: HashMap<String, HashSet<String>> = by_id
        .keys()
        .map(|id| (id.clone(), HashSet::new()))
        .collect();
    for p in pipelines {
        for pair in p.stages.windows(2) {
            let from = &pair[0].id;
            let to = &pair[1].id;
            if let Some(adj) = adjacency.get_mut(from)
                && adj.insert(to.clone())
            {
                *indegree.entry(to.clone()).or_insert(0) += 1;
            }
        }
    }

    let mut remaining: HashSet<String> = by_id.keys().cloned().collect();
    let mut ready: Vec<String> = remaining
        .iter()
        .filter(|id| indegree.get(*id).copied().unwrap_or(0) == 0)
        .cloned()
        .collect();
    let mut ordered: Vec<String> = Vec::new();

    while !ready.is_empty() {
        let next = ready
            .iter()
            .min_by_key(|id| first_seen.get(*id).copied().unwrap_or(usize::MAX))
            .cloned()
            .expect("ready is non-empty");
        ready.retain(|id| id != &next);
        ordered.push(next.clone());
        remaining.remove(&next);
        if let Some(succs) = adjacency.get(&next) {
            for succ in succs {
                if let Some(deg) = indegree.get_mut(succ) {
                    *deg -= 1;
                    if *deg == 0 {
                        ready.push(succ.clone());
                    }
                }
            }
        }
    }

    // Safe fallback: a cycle would leave nodes unprocessed (shouldn't happen
    // with well-formed pipeline files); append the remainder in first-seen
    // order rather than silently dropping stages.
    if !remaining.is_empty() {
        let mut rest: Vec<String> = remaining.into_iter().collect();
        rest.sort_by_key(|id| first_seen.get(id).copied().unwrap_or(0));
        ordered.extend(rest);
    }

    ordered
        .into_iter()
        .filter_map(|id| by_id.get(&id).cloned())
        .collect()
}

/// Compose the delimited `## Pipeline` markdown block for the chosen
/// pipeline(s), matching `composePipelineBlock`'s output for a fresh issue
/// (no `previousBlock` to merge manual lines from). Returns an empty string
/// when there is nothing to emit.
pub(crate) fn compose_pipeline_block(
    pipelines: &[McpPipeline],
    enabled_ids: &HashSet<String>,
    executor: Option<&str>,
) -> String {
    let executor_line = compose_executor_line(executor);
    let stages: Vec<McpPipelineStep> = canonical_stage_order(pipelines)
        .into_iter()
        .filter(|s| enabled_ids.contains(&s.id))
        .collect();

    if stages.is_empty() && executor_line.is_empty() {
        return String::new();
    }

    let heading = if pipelines.is_empty() {
        "## Pipeline".to_string()
    } else {
        format!(
            "## Pipeline: {}",
            pipelines
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(" + ")
        )
    };

    let mut lines: Vec<String> = vec![heading, String::new()];

    if !stages.is_empty() {
        lines.push(ORDER_INSTRUCTION.to_string());
        lines.push(String::new());
    }
    if !executor_line.is_empty() {
        lines.push(executor_line);
        if !stages.is_empty() {
            lines.push(String::new());
        }
    }
    for (i, s) in stages.iter().enumerate() {
        lines.push(format!("{}. {}", i + 1, s.prompt_fragment));
    }

    format!("{PIPELINE_START}\n{}\n{PIPELINE_END}", lines.join("\n"))
}

/// Append a freshly composed pipeline `block` to `description`, matching
/// `appendPipelineToDescription` for the create path (no existing block to
/// strip first). Returns `description` unchanged when `block` is empty.
pub(crate) fn append_pipeline_block(description: Option<String>, block: &str) -> Option<String> {
    if block.is_empty() {
        return description;
    }
    match description.as_deref().map(str::trim) {
        Some(base) if !base.is_empty() => Some(format!("{base}\n\n{block}")),
        _ => Some(block.to_string()),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn step(id: &str, prompt_fragment: &str, default_enabled: bool) -> McpPipelineStep {
        McpPipelineStep {
            id: id.to_string(),
            label: id.to_string(),
            prompt_fragment: prompt_fragment.to_string(),
            default_enabled,
            heavy: false,
        }
    }

    fn basic_pipeline() -> McpPipeline {
        McpPipeline {
            id: "basic".to_string(),
            name: "Basic".to_string(),
            description: None,
            stages: vec![
                step("spec", "Write a spec.", true),
                step("plan", "Write a plan.", true),
                step("code-review", "Review the code.", false),
            ],
        }
    }

    #[test]
    fn renders_enabled_stages_in_pipeline_order_not_selection_order() {
        let pipelines = vec![basic_pipeline()];
        let enabled: HashSet<String> = ["plan", "spec"].into_iter().map(String::from).collect();
        let block = compose_pipeline_block(&pipelines, &enabled, None);

        assert!(block.contains("## Pipeline: Basic"));
        assert!(block.contains(
            "Execute these stages in the order listed. Do not add, skip, or reorder stages."
        ));
        assert!(block.contains("1. Write a spec."));
        assert!(block.contains("2. Write a plan."));
        assert!(block.starts_with(PIPELINE_START));
        assert!(block.ends_with(PIPELINE_END));
    }

    #[test]
    fn executor_line_leads_before_stages() {
        let pipelines = vec![basic_pipeline()];
        let enabled: HashSet<String> = ["spec"].into_iter().map(String::from).collect();
        let block = compose_pipeline_block(&pipelines, &enabled, Some("CODEX"));

        let exec_idx = block.find("Run this task with the **CODEX**").unwrap();
        let stage_idx = block.find("1. Write a spec.").unwrap();
        assert!(stage_idx > exec_idx);
    }

    #[test]
    fn empty_selection_is_empty_string() {
        let pipelines = vec![basic_pipeline()];
        let block = compose_pipeline_block(&pipelines, &HashSet::new(), None);
        assert_eq!(block, "");
    }

    #[test]
    fn no_pipelines_with_executor_emits_executor_only_block() {
        let block = compose_pipeline_block(&[], &HashSet::new(), Some("CLAUDE_CODE"));
        assert!(block.contains("## Pipeline"));
        assert!(block.contains("Run this task with the **CLAUDE_CODE**"));
        assert!(!block.contains("1."));
    }

    #[test]
    fn no_pipelines_without_executor_is_empty() {
        let enabled: HashSet<String> = ["spec"].into_iter().map(String::from).collect();
        let block = compose_pipeline_block(&[], &enabled, None);
        assert_eq!(block, "");
    }

    #[test]
    fn merges_and_dedupes_a_stage_shared_by_two_pipelines() {
        let a = McpPipeline {
            id: "a".to_string(),
            name: "A".to_string(),
            description: None,
            stages: vec![
                step("spec", "Write a spec.", true),
                step("shared", "Shared.", true),
            ],
        };
        let b = McpPipeline {
            id: "b".to_string(),
            name: "B".to_string(),
            description: None,
            stages: vec![
                step("shared", "Shared.", true),
                step("wrap-up", "Wrap up.", true),
            ],
        };
        let ordered = canonical_stage_order(&[a, b]);
        let ids: Vec<&str> = ordered.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["spec", "shared", "wrap-up"]);
    }

    #[test]
    fn append_pipeline_block_joins_prose_and_block_with_blank_line() {
        let block = compose_pipeline_block(
            &[basic_pipeline()],
            &["spec".to_string()].into_iter().collect(),
            None,
        );
        let with_block = append_pipeline_block(Some("My task body.".to_string()), &block);
        let with_block = with_block.unwrap();
        assert!(with_block.contains("My task body."));
        assert!(with_block.contains("1. Write a spec."));
    }

    #[test]
    fn append_pipeline_block_noop_when_block_empty() {
        let result = append_pipeline_block(Some("My task body.".to_string()), "");
        assert_eq!(result, Some("My task body.".to_string()));

        let result = append_pipeline_block(None, "");
        assert_eq!(result, None);
    }
}
