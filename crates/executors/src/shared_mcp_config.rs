use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use ts_rs::TS;

use crate::{
    executors::{BaseCodingAgent, CodingAgent, StandardCodingAgentExecutor},
    mcp_config::{McpConfig, PRECONFIGURED_MCP_SERVERS, read_agent_config},
    profile::{ExecutorConfigs, ExecutorProfileId},
};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SharedMcpProfile {
    pub executor: BaseCodingAgent,
    pub display_name: String,
    pub supports_mcp: bool,
    pub config_path: Option<String>,
    pub servers_path: Vec<String>,
    pub read_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpTransportKind {
    Stdio,
    Http,
    Sse,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub struct McpServerDefinition {
    pub transport: McpTransportKind,
    pub value: Value,
    pub representable_in_form: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct NativeMcpSource {
    pub executor: BaseCodingAgent,
    pub config_path: String,
    pub server_name: String,
    pub entry: Value,
    pub normalized_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SharedMcpAssignment {
    pub executor: BaseCodingAgent,
    pub native_name: String,
    pub native_entry: Option<Value>,
    pub has_credentials: bool,
    pub representable: bool,
    pub incompatibility_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SharedMcpSourceKind {
    Reconciled,
    SingleProfile,
    New,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SharedMcpCompatibility {
    pub executor: BaseCodingAgent,
    pub compatible: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SharedMcpServer {
    pub name: String,
    pub definition: McpServerDefinition,
    pub assignments: Vec<SharedMcpAssignment>,
    pub source_kind: SharedMcpSourceKind,
    pub native_sources: Vec<NativeMcpSource>,
    pub compatibility: Vec<SharedMcpCompatibility>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SharedMcpConflictVariant {
    pub variant_id: String,
    pub definition: McpServerDefinition,
    pub assignments: Vec<SharedMcpAssignment>,
    pub native_sources: Vec<NativeMcpSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SharedMcpConflict {
    pub name: String,
    pub variants: Vec<SharedMcpConflictVariant>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SharedMcpProfileError {
    pub executor: BaseCodingAgent,
    pub config_path: Option<String>,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SharedMcpReadResponse {
    pub profiles: Vec<SharedMcpProfile>,
    pub servers: Vec<SharedMcpServer>,
    pub conflicts: Vec<SharedMcpConflict>,
    pub preconfigured: Value,
    pub read_errors: Vec<SharedMcpProfileError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SharedMcpServerInput {
    pub name: String,
    pub definition: McpServerDefinition,
    pub assignments: Vec<BaseCodingAgent>,
    #[serde(default)]
    pub native_overrides: HashMap<BaseCodingAgent, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SharedMcpConflictResolution {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SharedMcpWriteRequest {
    pub servers: Vec<SharedMcpServerInput>,
    #[serde(default)]
    pub resolved_conflicts: Vec<SharedMcpConflictResolution>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SharedMcpWriteStatus {
    Success,
    PartialFailure,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SharedMcpProfileWriteStatus {
    Success,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SharedMcpProfileWriteOutcome {
    pub executor: BaseCodingAgent,
    pub config_path: Option<String>,
    pub status: SharedMcpProfileWriteStatus,
    pub affected_servers: Vec<String>,
    pub message: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SharedMcpWriteResponse {
    pub status: SharedMcpWriteStatus,
    pub outcomes: Vec<SharedMcpProfileWriteOutcome>,
    pub servers: Vec<SharedMcpServer>,
    pub conflicts: Vec<SharedMcpConflict>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SharedMcpTestTarget {
    pub server_name: String,
    pub executor: BaseCodingAgent,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, Default)]
pub struct SharedMcpTestRequest {
    #[serde(default)]
    pub targets: Vec<SharedMcpTestTarget>,
}

#[derive(Debug, Clone)]
pub struct NativeProfileSnapshot {
    pub profile: SharedMcpProfile,
    pub config_path: Option<PathBuf>,
    pub mcp_config: McpConfig,
    pub servers: HashMap<String, Value>,
}

fn agent_for(executor: BaseCodingAgent) -> Option<CodingAgent> {
    ExecutorConfigs::get_cached().get_coding_agent(&ExecutorProfileId::new(executor))
}

pub async fn load_native_snapshots() -> Vec<NativeProfileSnapshot> {
    let profiles = ExecutorConfigs::get_cached();
    let mut snapshots = Vec::new();
    let mut executors: Vec<_> = profiles.executors.keys().copied().collect();
    executors.sort_by_key(|e| e.to_string());

    for executor in executors {
        let Some(agent) = profiles.get_coding_agent(&ExecutorProfileId::new(executor)) else {
            continue;
        };
        let supports_mcp = agent.supports_mcp();
        if !supports_mcp {
            continue;
        }
        let config_path = agent.default_mcp_config_path();
        let mut mcp_config = agent.get_mcp_config();
        let mut servers = HashMap::new();
        let mut read_error = None;

        if let Some(path) = &config_path {
            match read_agent_config(path, &mcp_config).await {
                Ok(raw_config) => {
                    servers = get_servers_from_config_path(&raw_config, &mcp_config.servers_path);
                    mcp_config.set_servers(servers.clone());
                }
                Err(e) => read_error = Some(e.to_string()),
            }
        }

        snapshots.push(NativeProfileSnapshot {
            profile: SharedMcpProfile {
                executor,
                display_name: executor.to_string(),
                supports_mcp,
                config_path: config_path
                    .as_ref()
                    .map(|path| path.to_string_lossy().to_string()),
                servers_path: mcp_config.servers_path.clone(),
                read_error,
            },
            config_path,
            mcp_config,
            servers,
        });
    }
    snapshots
}

pub async fn load_shared_mcp_config() -> SharedMcpReadResponse {
    reconcile_snapshots(load_native_snapshots().await)
}

pub fn reconcile_snapshots(snapshots: Vec<NativeProfileSnapshot>) -> SharedMcpReadResponse {
    let read_errors = snapshots
        .iter()
        .filter_map(|snapshot| {
            snapshot
                .profile
                .read_error
                .as_ref()
                .map(|error| SharedMcpProfileError {
                    executor: snapshot.profile.executor,
                    config_path: snapshot.profile.config_path.clone(),
                    error: error.clone(),
                })
        })
        .collect::<Vec<_>>();
    let profiles = snapshots
        .iter()
        .map(|snapshot| snapshot.profile.clone())
        .collect::<Vec<_>>();

    let mut by_name: BTreeMap<String, Vec<(NativeMcpSource, McpServerDefinition)>> =
        BTreeMap::new();
    for snapshot in &snapshots {
        let config_path = snapshot
            .profile
            .config_path
            .clone()
            .unwrap_or_else(|| "<unknown>".to_string());
        for (server_name, entry) in &snapshot.servers {
            let definition = canonical_definition(entry);
            let fingerprint = normalized_fingerprint(&definition);
            by_name.entry(server_name.clone()).or_default().push((
                NativeMcpSource {
                    executor: snapshot.profile.executor,
                    config_path: config_path.clone(),
                    server_name: server_name.clone(),
                    entry: entry.clone(),
                    normalized_fingerprint: fingerprint,
                },
                definition,
            ));
        }
    }

    let mut servers = Vec::new();
    let mut conflicts = Vec::new();
    for (name, sources) in by_name {
        let mut variants: BTreeMap<String, Vec<(NativeMcpSource, McpServerDefinition)>> =
            BTreeMap::new();
        for (source, definition) in sources {
            let key = normalized_fingerprint(&definition).unwrap_or_else(|| {
                format!("custom:{}:{}", source.executor, stable_json(&source.entry))
            });
            variants.entry(key).or_default().push((source, definition));
        }

        if variants.len() == 1 {
            let (_, group) = variants.into_iter().next().expect("variant exists");
            let definition = group[0].1.clone();
            let native_sources = group
                .iter()
                .map(|(source, _)| source.clone())
                .collect::<Vec<_>>();
            let assignments = native_sources
                .iter()
                .map(|source| assignment_from_source(source, &definition))
                .collect::<Vec<_>>();
            servers.push(SharedMcpServer {
                name,
                definition: definition.clone(),
                assignments,
                source_kind: if native_sources.len() > 1 {
                    SharedMcpSourceKind::Reconciled
                } else if definition.transport == McpTransportKind::Unknown {
                    SharedMcpSourceKind::Custom
                } else {
                    SharedMcpSourceKind::SingleProfile
                },
                native_sources,
                compatibility: compatibility_for_profiles(&profiles, &definition),
            });
        } else {
            let variants = variants
                .into_iter()
                .enumerate()
                .map(|(idx, (_, group))| {
                    let definition = group[0].1.clone();
                    let native_sources = group
                        .iter()
                        .map(|(source, _)| source.clone())
                        .collect::<Vec<_>>();
                    let assignments = native_sources
                        .iter()
                        .map(|source| assignment_from_source(source, &definition))
                        .collect();
                    SharedMcpConflictVariant {
                        variant_id: format!("variant-{}", idx + 1),
                        definition,
                        assignments,
                        native_sources,
                    }
                })
                .collect();
            conflicts.push(SharedMcpConflict {
                name: name.clone(),
                variants,
                message: format!(
                    "MCP server `{name}` has different definitions across assigned profiles"
                ),
            });
        }
    }

    SharedMcpReadResponse {
        profiles,
        servers,
        conflicts,
        preconfigured: PRECONFIGURED_MCP_SERVERS.clone(),
        read_errors,
    }
}

fn assignment_from_source(
    source: &NativeMcpSource,
    definition: &McpServerDefinition,
) -> SharedMcpAssignment {
    let reason = incompatibility_reason(source.executor, definition);
    SharedMcpAssignment {
        executor: source.executor,
        native_name: source.server_name.clone(),
        native_entry: Some(source.entry.clone()),
        has_credentials: has_credentials(&source.entry),
        representable: reason.is_none(),
        incompatibility_reason: reason,
    }
}

pub fn canonical_definition(entry: &Value) -> McpServerDefinition {
    let Some(obj) = entry.as_object() else {
        return McpServerDefinition {
            transport: McpTransportKind::Unknown,
            value: entry.clone(),
            representable_in_form: false,
        };
    };
    if obj.get("enabled").and_then(Value::as_bool) == Some(false) {
        return McpServerDefinition {
            transport: McpTransportKind::Unknown,
            value: entry.clone(),
            representable_in_form: false,
        };
    }
    let type_str = obj.get("type").and_then(Value::as_str);

    if type_str == Some("local")
        && let Some(parts) = obj.get("command").and_then(Value::as_array)
    {
        let command = parts.first().and_then(Value::as_str).unwrap_or_default();
        let args = parts
            .iter()
            .skip(1)
            .filter_map(Value::as_str)
            .map(|arg| Value::String(arg.to_string()))
            .collect();
        return McpServerDefinition {
            transport: McpTransportKind::Stdio,
            value: compact_object([
                ("command", Value::String(command.to_string())),
                ("args", Value::Array(args)),
                ("env", normalize_string_map(obj, &["env", "environment"])),
            ]),
            representable_in_form: !command.is_empty(),
        };
    }

    if let Some(command) = obj.get("command").and_then(Value::as_str)
        && !matches!(type_str, Some("http") | Some("sse") | Some("remote"))
    {
        return McpServerDefinition {
            transport: McpTransportKind::Stdio,
            value: compact_object([
                ("command", Value::String(command.to_string())),
                (
                    "args",
                    Value::Array(
                        obj.get("args")
                            .and_then(Value::as_array)
                            .cloned()
                            .unwrap_or_default(),
                    ),
                ),
                ("env", normalize_string_map(obj, &["env", "environment"])),
            ]),
            representable_in_form: true,
        };
    }

    if let Some(url) = obj
        .get("url")
        .and_then(Value::as_str)
        .or_else(|| obj.get("httpUrl").and_then(Value::as_str))
    {
        let transport = if type_str == Some("sse") || url.trim_end_matches('/').ends_with("/sse") {
            McpTransportKind::Sse
        } else {
            McpTransportKind::Http
        };
        return McpServerDefinition {
            transport,
            value: compact_object([
                ("url", Value::String(url.to_string())),
                ("headers", normalize_string_map(obj, &["headers"])),
            ]),
            representable_in_form: true,
        };
    }

    McpServerDefinition {
        transport: McpTransportKind::Unknown,
        value: entry.clone(),
        representable_in_form: false,
    }
}

fn compact_object<const N: usize>(entries: [(&str, Value); N]) -> Value {
    let mut map = Map::new();
    for (key, value) in entries {
        if matches!(&value, Value::Array(v) if v.is_empty())
            || matches!(&value, Value::Object(v) if v.is_empty())
            || matches!(&value, Value::String(v) if v.is_empty())
        {
            continue;
        }
        map.insert(key.to_string(), value);
    }
    Value::Object(map)
}

fn normalize_string_map(obj: &Map<String, Value>, keys: &[&str]) -> Value {
    for key in keys {
        if let Some(Value::Object(map)) = obj.get(*key) {
            let mut out = Map::new();
            for (k, v) in map {
                if let Some(s) = v.as_str() {
                    out.insert(k.clone(), Value::String(s.to_string()));
                }
            }
            return Value::Object(out);
        }
    }
    Value::Object(Map::new())
}

pub fn normalized_fingerprint(definition: &McpServerDefinition) -> Option<String> {
    if definition.transport == McpTransportKind::Unknown {
        None
    } else {
        Some(format!(
            "{:?}:{}",
            definition.transport,
            stable_json(&definition.value)
        ))
    }
}

fn stable_json(value: &Value) -> String {
    fn sorted(value: &Value) -> Value {
        match value {
            Value::Object(map) => {
                let mut keys: Vec<_> = map.keys().collect();
                keys.sort();
                Value::Object(
                    keys.into_iter()
                        .map(|key| (key.clone(), sorted(&map[key])))
                        .collect(),
                )
            }
            Value::Array(values) => Value::Array(values.iter().map(sorted).collect()),
            _ => value.clone(),
        }
    }
    serde_json::to_string(&sorted(value)).unwrap_or_else(|_| "null".to_string())
}

fn has_credentials(entry: &Value) -> bool {
    entry
        .get("headers")
        .and_then(Value::as_object)
        .is_some_and(|headers| {
            headers
                .keys()
                .any(|key| key.eq_ignore_ascii_case("authorization"))
        })
}

pub fn incompatibility_reason(
    executor: BaseCodingAgent,
    definition: &McpServerDefinition,
) -> Option<String> {
    if matches!(executor, BaseCodingAgent::Codex)
        && !matches!(definition.transport, McpTransportKind::Stdio)
    {
        return Some("Codex supports stdio MCP servers only".to_string());
    }
    if matches!(definition.transport, McpTransportKind::Unknown) {
        return Some("This server shape cannot be shared safely".to_string());
    }
    None
}

pub fn compatibility_for_profiles(
    profiles: &[SharedMcpProfile],
    definition: &McpServerDefinition,
) -> Vec<SharedMcpCompatibility> {
    profiles
        .iter()
        .map(|profile| {
            let reason = incompatibility_reason(profile.executor, definition);
            SharedMcpCompatibility {
                executor: profile.executor,
                compatible: reason.is_none(),
                reason,
            }
        })
        .collect()
}

pub fn materialize_definition(
    executor: BaseCodingAgent,
    definition: &McpServerDefinition,
    native_override: Option<&Value>,
) -> Result<Value, String> {
    if let Some(reason) = incompatibility_reason(executor, definition) {
        return Err(reason);
    }
    if let Some(override_value) = native_override {
        return Ok(override_value.clone());
    }
    let Some(obj) = definition.value.as_object() else {
        return Err("MCP server definition is not an object".to_string());
    };
    match definition.transport {
        McpTransportKind::Stdio => {
            let mut out = Map::new();
            out.insert(
                "command".to_string(),
                obj.get("command")
                    .cloned()
                    .unwrap_or(Value::String(String::new())),
            );
            if let Some(args) = obj
                .get("args")
                .filter(|v| !v.as_array().is_some_and(Vec::is_empty))
            {
                out.insert("args".to_string(), args.clone());
            }
            if let Some(env) = obj
                .get("env")
                .filter(|v| !v.as_object().is_some_and(Map::is_empty))
            {
                out.insert("env".to_string(), env.clone());
            }
            if matches!(executor, BaseCodingAgent::Opencode) {
                let mut parts = Vec::new();
                if let Some(command) = out
                    .remove("command")
                    .and_then(|v| v.as_str().map(str::to_string))
                {
                    parts.push(Value::String(command));
                }
                if let Some(Value::Array(args)) = out.remove("args") {
                    parts.extend(args);
                }
                out.insert("type".to_string(), Value::String("local".to_string()));
                out.insert("command".to_string(), Value::Array(parts));
                out.insert("enabled".to_string(), Value::Bool(true));
            }
            Ok(Value::Object(out))
        }
        McpTransportKind::Http | McpTransportKind::Sse => {
            let mut out = Map::new();
            let url = obj
                .get("url")
                .cloned()
                .unwrap_or(Value::String(String::new()));
            if matches!(
                executor,
                BaseCodingAgent::Gemini | BaseCodingAgent::QwenCode
            ) {
                out.insert("httpUrl".to_string(), url);
            } else {
                if matches!(executor, BaseCodingAgent::Opencode) {
                    out.insert("type".to_string(), Value::String("remote".to_string()));
                    out.insert("enabled".to_string(), Value::Bool(true));
                } else if !matches!(executor, BaseCodingAgent::CursorAgent) {
                    out.insert(
                        "type".to_string(),
                        Value::String(
                            if definition.transport == McpTransportKind::Sse {
                                "sse"
                            } else {
                                "http"
                            }
                            .to_string(),
                        ),
                    );
                }
                out.insert("url".to_string(), url);
            }
            if let Some(headers) = obj
                .get("headers")
                .filter(|v| !v.as_object().is_some_and(Map::is_empty))
            {
                out.insert("headers".to_string(), headers.clone());
            }
            Ok(Value::Object(out))
        }
        McpTransportKind::Unknown => Err("Unknown MCP transport cannot be shared".to_string()),
    }
}

pub fn plan_servers_for_executor(
    executor: BaseCodingAgent,
    current: &HashMap<String, Value>,
    request: &SharedMcpWriteRequest,
) -> Result<(HashMap<String, Value>, Vec<String>), String> {
    let mut next = current.clone();
    let mut affected = Vec::new();
    for server in &request.servers {
        if server.assignments.contains(&executor) {
            let entry = materialize_definition(
                executor,
                &server.definition,
                server.native_overrides.get(&executor),
            )?;
            next.insert(server.name.clone(), entry);
            affected.push(server.name.clone());
        } else if next.remove(&server.name).is_some() {
            affected.push(server.name.clone());
        }
    }
    for (name, entry) in current {
        let assigned = request.servers.iter().any(|server| {
            server.name == *name && server.assignments.contains(&executor)
        });
        if !assigned && canonical_definition(entry).transport != McpTransportKind::Unknown {
            if next.remove(name).is_some() && !affected.contains(name) {
                affected.push(name.clone());
            }
        }
    }
    Ok((next, affected))
}

pub fn validate_write_request(request: &SharedMcpWriteRequest) -> Result<(), String> {
    let mut names = HashSet::new();
    for server in &request.servers {
        if server.name.trim().is_empty() {
            return Err("MCP server names must not be empty".to_string());
        }
        if !names.insert(server.name.clone()) {
            return Err(format!("MCP server `{}` is duplicated", server.name));
        }
        if server.assignments.is_empty() {
            return Err(format!(
                "MCP server `{}` must have at least one assignment",
                server.name
            ));
        }
        for executor in &server.assignments {
            if agent_for(*executor).is_none() {
                return Err(format!("Executor `{executor}` was not found"));
            }
            if let Some(reason) = incompatibility_reason(*executor, &server.definition) {
                return Err(format!(
                    "MCP server `{}` cannot be assigned to `{}`: {}",
                    server.name, executor, reason
                ));
            }
        }
    }
    Ok(())
}

fn get_servers_from_config_path(raw_config: &Value, path: &[String]) -> HashMap<String, Value> {
    let mut current = raw_config;
    for part in path {
        current = match current.get(part) {
            Some(val) => val,
            None => return HashMap::new(),
        };
    }
    current
        .as_object()
        .map(|servers| {
            servers
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn snapshot(
        executor: BaseCodingAgent,
        servers: HashMap<String, Value>,
    ) -> NativeProfileSnapshot {
        NativeProfileSnapshot {
            profile: SharedMcpProfile {
                executor,
                display_name: executor.to_string(),
                supports_mcp: true,
                config_path: Some(format!("/tmp/{executor}.json")),
                servers_path: vec!["mcpServers".to_string()],
                read_error: None,
            },
            config_path: Some(PathBuf::from(format!("/tmp/{executor}.json"))),
            mcp_config: McpConfig::new(vec!["mcpServers".to_string()], json!({}), json!({}), false),
            servers,
        }
    }

    #[test]
    fn reconciles_identical_same_name_entries() {
        let entry = json!({"command":"npx","args":["-y","server"]});
        let response = reconcile_snapshots(vec![
            snapshot(
                BaseCodingAgent::ClaudeCode,
                HashMap::from([("tools".to_string(), entry.clone())]),
            ),
            snapshot(
                BaseCodingAgent::Gemini,
                HashMap::from([("tools".to_string(), entry)]),
            ),
        ]);
        assert_eq!(response.servers.len(), 1);
        assert_eq!(response.conflicts.len(), 0);
        assert_eq!(response.servers[0].assignments.len(), 2);
    }

    #[test]
    fn detects_incompatible_same_name_conflicts() {
        let response = reconcile_snapshots(vec![
            snapshot(
                BaseCodingAgent::ClaudeCode,
                HashMap::from([("tools".to_string(), json!({"command":"a"}))]),
            ),
            snapshot(
                BaseCodingAgent::Gemini,
                HashMap::from([("tools".to_string(), json!({"command":"b"}))]),
            ),
        ]);
        assert_eq!(response.servers.len(), 0);
        assert_eq!(response.conflicts.len(), 1);
        assert_eq!(response.conflicts[0].variants.len(), 2);
    }

    #[test]
    fn blocks_url_assignments_to_codex() {
        let definition =
            canonical_definition(&json!({"type":"http","url":"https://example.test/mcp"}));
        assert!(incompatibility_reason(BaseCodingAgent::Codex, &definition).is_some());
        assert!(materialize_definition(BaseCodingAgent::Codex, &definition, None).is_err());
    }

    #[test]
    fn preserves_unrelated_native_servers_in_write_plan() {
        let request = SharedMcpWriteRequest {
            servers: vec![SharedMcpServerInput {
                name: "shared".to_string(),
                definition: canonical_definition(&json!({"command":"npx"})),
                assignments: vec![BaseCodingAgent::ClaudeCode],
                native_overrides: HashMap::new(),
            }],
            resolved_conflicts: Vec::new(),
        };
        let current = HashMap::from([
            ("other".to_string(), json!({"command":"keep"})),
            ("shared".to_string(), json!({"command":"old"})),
        ]);
        let (next, affected) =
            plan_servers_for_executor(BaseCodingAgent::ClaudeCode, &current, &request).unwrap();
        assert_eq!(next["other"], json!({"command":"keep"}));
        assert_eq!(next["shared"], json!({"command":"npx"}));
        assert_eq!(affected, vec!["shared"]);
    }

    #[test]
    fn unassigned_server_is_removed_only_for_that_executor() {
        let request = SharedMcpWriteRequest {
            servers: vec![SharedMcpServerInput {
                name: "shared".to_string(),
                definition: canonical_definition(&json!({"command":"npx"})),
                assignments: vec![BaseCodingAgent::Gemini],
                native_overrides: HashMap::new(),
            }],
            resolved_conflicts: Vec::new(),
        };
        let current = HashMap::from([
            ("other".to_string(), json!({"command":"keep"})),
            ("shared".to_string(), json!({"command":"old"})),
        ]);
        let (next, affected) =
            plan_servers_for_executor(BaseCodingAgent::ClaudeCode, &current, &request).unwrap();
        assert_eq!(next.get("shared"), None);
        assert_eq!(next["other"], json!({"command":"keep"}));
        assert_eq!(affected, vec!["shared"]);
    }
}
