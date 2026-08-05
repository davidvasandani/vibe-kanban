use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::{Path, PathBuf},
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

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SharedMcpAuthMode {
    SharedGateway,
    AgentNative,
    ExplicitHeader,
    None,
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
    #[serde(default)]
    pub display_name: Option<String>,
    pub definition: McpServerDefinition,
    pub assignments: Vec<SharedMcpAssignment>,
    pub source_kind: SharedMcpSourceKind,
    pub native_sources: Vec<NativeMcpSource>,
    pub compatibility: Vec<SharedMcpCompatibility>,
    pub auth_mode: SharedMcpAuthMode,
    pub gateway_status: Option<String>,
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
    #[serde(default)]
    pub display_name: Option<String>,
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
    pub metadata_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SharedMcpServerInput {
    pub name: String,
    #[serde(default)]
    pub display_name: Option<String>,
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
    pub removed_servers: Vec<String>,
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
    pub metadata_error: Option<String>,
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

/// MCP server names are protocol identifiers, not display labels. Keep this in
/// sync with the validation performed by MCP clients such as Codex.
pub fn is_valid_server_identifier(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

/// Produce an actionable, protocol-safe suggestion without silently changing
/// the key that the user supplied.
pub fn suggested_server_identifier(name: &str) -> String {
    let mut suggestion = String::new();
    let mut previous_was_separator = false;

    for character in name.trim().chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
            suggestion.push(character.to_ascii_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator && !suggestion.is_empty() {
            suggestion.push('_');
            previous_was_separator = true;
        }
    }

    while suggestion.ends_with('_') {
        suggestion.pop();
    }
    if suggestion.is_empty() {
        "mcp_server".to_string()
    } else {
        suggestion
    }
}

pub fn validate_server_identifiers<'a>(
    names: impl IntoIterator<Item = &'a str>,
) -> Result<(), String> {
    for name in names {
        if !is_valid_server_identifier(name) {
            return Err(format!(
                "Invalid MCP server identifier `{name}`: identifiers must match \
                 ^[a-zA-Z0-9_-]+$. Use `{}` as the identifier and keep `{name}` \
                 as the display label.",
                suggested_server_identifier(name)
            ));
        }
    }
    Ok(())
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
    let mut response = reconcile_snapshots(load_native_snapshots().await);
    let labels = match load_display_labels().await {
        Ok(labels) => labels,
        Err(error) => {
            tracing::warn!(%error, "MCP display labels are unavailable");
            response.metadata_error = Some(error);
            BTreeMap::new()
        }
    };
    attach_display_labels(&mut response, &labels);
    response
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct SharedMcpDisplayLabels {
    #[serde(default = "display_label_store_version")]
    version: u8,
    #[serde(default)]
    labels: BTreeMap<String, String>,
}

fn display_label_store_version() -> u8 {
    1
}

fn display_labels_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("vibe-kanban")
        .join("mcp-display-labels.json")
}

async fn load_display_labels_from(path: &Path) -> Result<BTreeMap<String, String>, String> {
    let content = match tokio::fs::read(path).await {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => return Err(format!("failed to read MCP display labels: {error}")),
    };
    let store: SharedMcpDisplayLabels = serde_json::from_slice(&content)
        .map_err(|error| format!("failed to parse MCP display labels: {error}"))?;
    if store.version != display_label_store_version() {
        return Err(format!(
            "unsupported MCP display-label store version {}",
            store.version
        ));
    }
    Ok(store.labels)
}

async fn load_display_labels() -> Result<BTreeMap<String, String>, String> {
    load_display_labels_from(&display_labels_path()).await
}

fn normalized_display_name(identifier: &str, display_name: Option<&str>) -> Option<String> {
    display_name
        .map(str::trim)
        .filter(|label| !label.is_empty() && *label != identifier)
        .map(str::to_string)
}

fn attach_display_labels(response: &mut SharedMcpReadResponse, labels: &BTreeMap<String, String>) {
    for server in &mut response.servers {
        server.display_name = labels.get(&server.name).cloned();
    }
    for conflict in &mut response.conflicts {
        conflict.display_name = labels.get(&conflict.name).cloned();
    }
}

async fn write_display_labels_to(
    path: &Path,
    labels: &BTreeMap<String, String>,
) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "MCP display-label path has no parent directory".to_string())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|error| format!("failed to create MCP display-label directory: {error}"))?;
    let store = SharedMcpDisplayLabels {
        version: display_label_store_version(),
        labels: labels.clone(),
    };
    let content = serde_json::to_vec_pretty(&store)
        .map_err(|error| format!("failed to serialize MCP display labels: {error}"))?;
    let staged = path.with_extension(format!("json.{}.tmp", uuid::Uuid::new_v4()));
    tokio::fs::write(&staged, content)
        .await
        .map_err(|error| format!("failed to stage MCP display labels: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o600))
            .await
            .map_err(|error| format!("failed to protect MCP display labels: {error}"))?;
    }
    // Windows rename does not replace an existing destination. Move the old
    // sidecar to a backup first, restoring it if installing the staged file
    // fails, so a transient replacement error cannot discard existing labels.
    #[cfg(windows)]
    let backup = if tokio::fs::metadata(path).await.is_ok() {
        let backup = path.with_extension(format!("json.{}.bak", uuid::Uuid::new_v4()));
        tokio::fs::rename(path, &backup)
            .await
            .map_err(|error| format!("failed to preserve old MCP display labels: {error}"))?;
        Some(backup)
    } else {
        None
    };
    if let Err(error) = tokio::fs::rename(&staged, path).await {
        let _ = tokio::fs::remove_file(&staged).await;
        #[cfg(windows)]
        if let Some(backup) = &backup {
            if let Err(restore_error) = tokio::fs::rename(backup, path).await {
                return Err(format!(
                    "failed to replace MCP display labels: {error}; failed to restore prior labels: {restore_error}"
                ));
            }
        }
        return Err(format!("failed to replace MCP display labels: {error}"));
    }
    #[cfg(windows)]
    if let Some(backup) = backup {
        let _ = tokio::fs::remove_file(backup).await;
    }
    Ok(())
}

pub async fn persist_display_labels(
    request: &SharedMcpWriteRequest,
    existing_identifiers: &HashSet<String>,
    updatable_identifiers: Option<&HashSet<String>>,
) -> Result<(), String> {
    let current = load_display_labels().await.unwrap_or_else(|error| {
        tracing::warn!(%error, "replacing unavailable MCP display-label metadata");
        BTreeMap::new()
    });
    let labels = merged_display_labels(
        current,
        request,
        existing_identifiers,
        updatable_identifiers,
    );
    write_display_labels_to(&display_labels_path(), &labels).await
}

fn merged_display_labels(
    mut labels: BTreeMap<String, String>,
    request: &SharedMcpWriteRequest,
    existing_identifiers: &HashSet<String>,
    updatable_identifiers: Option<&HashSet<String>>,
) -> BTreeMap<String, String> {
    labels.retain(|identifier, _| existing_identifiers.contains(identifier));
    for server in &request.servers {
        if !existing_identifiers.contains(&server.name)
            || updatable_identifiers.is_some_and(|identifiers| !identifiers.contains(&server.name))
        {
            continue;
        }
        match normalized_display_name(&server.name, server.display_name.as_deref()) {
            Some(label) => {
                labels.insert(server.name.clone(), label);
            }
            None => {
                labels.remove(&server.name);
            }
        }
    }
    labels
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
            let definition = canonical_definition_for_server(server_name, entry);
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
            let definition = redact_gateway_definition(group[0].1.clone());
            let native_sources = group
                .iter()
                .map(|(source, _)| redact_gateway_source(source.clone()))
                .collect::<Vec<_>>();
            let assignments = native_sources
                .iter()
                .map(|source| assignment_from_source(source, &definition))
                .collect::<Vec<_>>();
            servers.push(SharedMcpServer {
                name,
                display_name: None,
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
                auth_mode: auth_mode(&definition),
                gateway_status: None,
            });
        } else {
            let variants = variants
                .into_iter()
                .enumerate()
                .map(|(idx, (_, group))| {
                    let definition = redact_gateway_definition(group[0].1.clone());
                    let native_sources = group
                        .iter()
                        .map(|(source, _)| redact_gateway_source(source.clone()))
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
                display_name: None,
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
        metadata_error: None,
    }
}

fn gateway_url(definition: &McpServerDefinition) -> Option<&str> {
    definition
        .value
        .get("url")
        .and_then(Value::as_str)
        .filter(|url| url.contains("/mcp-gateway/"))
}

fn auth_mode(definition: &McpServerDefinition) -> SharedMcpAuthMode {
    if gateway_url(definition).is_some() {
        return SharedMcpAuthMode::SharedGateway;
    }
    let has_auth = definition
        .value
        .get("headers")
        .and_then(Value::as_object)
        .is_some_and(|headers| {
            headers
                .keys()
                .any(|key| key.eq_ignore_ascii_case("authorization"))
        });
    if has_auth {
        SharedMcpAuthMode::ExplicitHeader
    } else if matches!(
        definition.transport,
        McpTransportKind::Http | McpTransportKind::Sse
    ) {
        SharedMcpAuthMode::AgentNative
    } else {
        SharedMcpAuthMode::None
    }
}

fn redact_gateway_definition(mut definition: McpServerDefinition) -> McpServerDefinition {
    if gateway_url(&definition).is_some()
        && let Some(headers) = definition
            .value
            .get_mut("headers")
            .and_then(Value::as_object_mut)
    {
        for (name, value) in headers {
            if name.eq_ignore_ascii_case("authorization") {
                *value = Value::String("Bearer [REDACTED]".to_string());
            }
        }
    }
    definition
}

fn redact_gateway_source(mut source: NativeMcpSource) -> NativeMcpSource {
    let definition = canonical_definition(&source.entry);
    if gateway_url(&definition).is_some() {
        for key in ["headers", "http_headers"] {
            if let Some(headers) = source.entry.get_mut(key).and_then(Value::as_object_mut) {
                for (name, value) in headers {
                    if name.eq_ignore_ascii_case("authorization") {
                        *value = Value::String("Bearer [REDACTED]".to_string());
                    }
                }
            }
        }
    }
    source
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
                (
                    "headers",
                    normalize_string_map(obj, &["headers", "http_headers"]),
                ),
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

fn canonical_definition_for_server(name: &str, entry: &Value) -> McpServerDefinition {
    let mut definition = canonical_definition(entry);
    if name != "slack" || !is_legacy_bundled_slack_definition(&definition) {
        return definition;
    }

    let Some(current_entry) = PRECONFIGURED_MCP_SERVERS.get("slack") else {
        return definition;
    };
    let current = canonical_definition(current_entry);
    let Some(env) = definition.value.get("env").cloned() else {
        return definition;
    };

    definition = current;
    if let Some(value) = definition.value.as_object_mut() {
        value.insert("env".to_string(), env);
    }
    definition
}

fn is_legacy_bundled_slack_definition(definition: &McpServerDefinition) -> bool {
    definition.transport == McpTransportKind::Stdio
        && definition.value.get("command").and_then(Value::as_str) == Some("npx")
        && definition.value.get("args")
            == Some(&serde_json::json!([
                "-y",
                "slack-mcp-server@latest",
                "--transport",
                "stdio"
            ]))
        && definition
            .value
            .get("env")
            .and_then(Value::as_object)
            .is_some_and(|env| env.len() == 1 && env.contains_key("SLACK_MCP_XOXP_TOKEN"))
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
        .or_else(|| entry.get("http_headers"))
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
    if matches!(executor, BaseCodingAgent::Codex | BaseCodingAgent::Grok)
        && matches!(definition.transport, McpTransportKind::Sse)
    {
        return Some(format!(
            "{executor} supports stdio and streamable HTTP MCP servers, not legacy SSE"
        ));
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
                } else if !matches!(
                    executor,
                    BaseCodingAgent::CursorAgent | BaseCodingAgent::Codex | BaseCodingAgent::Grok
                ) {
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
                out.insert(
                    if matches!(executor, BaseCodingAgent::Codex) {
                        "http_headers"
                    } else {
                        "headers"
                    }
                    .to_string(),
                    headers.clone(),
                );
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
            let mut entry = materialize_definition(
                executor,
                &server.definition,
                server.native_overrides.get(&executor),
            )?;
            preserve_gateway_capability(current, &server.name, &mut entry);
            next.insert(server.name.clone(), entry);
            affected.push(server.name.clone());
        } else if next.remove(&server.name).is_some() {
            affected.push(server.name.clone());
        }
    }
    for name in &request.removed_servers {
        if next.remove(name).is_some() && !affected.contains(name) {
            affected.push(name.clone());
        }
    }
    Ok((next, affected))
}

fn preserve_gateway_capability(
    current_servers: &HashMap<String, Value>,
    current_name: &str,
    next: &mut Value,
) {
    let next_definition = canonical_definition(next);
    let Some(next_gateway_url) = gateway_url(&next_definition) else {
        return;
    };
    let current = current_servers.get(current_name).or_else(|| {
        current_servers
            .values()
            .find(|entry| gateway_url(&canonical_definition(entry)) == Some(next_gateway_url))
    });
    let Some(current) = current else { return };
    let current_definition = canonical_definition(current);
    let current_auth = current_definition
        .value
        .get("headers")
        .and_then(Value::as_object)
        .and_then(|headers| {
            headers
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case("authorization"))
        })
        .map(|(_, value)| value.clone());
    for key in ["headers", "http_headers"] {
        if let Some(headers) = next.get_mut(key).and_then(Value::as_object_mut) {
            for (name, value) in headers {
                if name.eq_ignore_ascii_case("authorization")
                    && value.as_str() == Some("Bearer [REDACTED]")
                    && let Some(current_auth) = &current_auth
                {
                    *value = current_auth.clone();
                }
            }
        }
    }
}

pub fn validate_write_request(request: &SharedMcpWriteRequest) -> Result<(), String> {
    validate_server_identifiers(request.servers.iter().map(|server| server.name.as_str()))?;

    let mut names = HashSet::new();
    for server in &request.servers {
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

    const SLACK_MCP_INSTALL_SPEC: &str = "https://github.com/davidvasandani/slack-mcp-server/releases/download/v1.3.0-vk.2/slack-mcp-server-vk-1.3.0-vk.2.tgz";

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

    fn toml_snapshot(
        executor: BaseCodingAgent,
        server_name: &str,
        entry_toml: &str,
    ) -> NativeProfileSnapshot {
        let toml_value: toml::Value = toml::from_str(entry_toml).unwrap();
        let entry = serde_json::to_value(toml_value).unwrap();
        NativeProfileSnapshot {
            profile: SharedMcpProfile {
                executor,
                display_name: executor.to_string(),
                supports_mcp: true,
                config_path: Some(format!("/tmp/{executor}.toml")),
                servers_path: vec!["mcp_servers".to_string()],
                read_error: None,
            },
            config_path: Some(PathBuf::from(format!("/tmp/{executor}.toml"))),
            mcp_config: McpConfig::new(
                vec!["mcp_servers".to_string()],
                json!({"mcp_servers": {}}),
                json!({}),
                true,
            ),
            servers: HashMap::from([(server_name.to_string(), entry)]),
        }
    }

    fn json_snapshot(
        executor: BaseCodingAgent,
        server_name: &str,
        entry: Value,
    ) -> NativeProfileSnapshot {
        snapshot(executor, HashMap::from([(server_name.to_string(), entry)]))
    }

    fn slack_json_entry_with_spec(token: &str, install_spec: &str) -> Value {
        json!({
            "command": "npx",
            "args": ["-y", install_spec, "--transport", "stdio"],
            "env": {
                "SLACK_MCP_XOXP_TOKEN": token
            }
        })
    }

    fn slack_json_entry(token: &str) -> Value {
        slack_json_entry_with_spec(token, SLACK_MCP_INSTALL_SPEC)
    }

    fn slack_toml_entry_with_spec(token: &str, install_spec: &str) -> String {
        format!(
            r#"
command = "npx"
args = ["-y", "{install_spec}", "--transport", "stdio"]

[env]
SLACK_MCP_XOXP_TOKEN = "{token}"
"#
        )
    }

    fn slack_toml_entry(token: &str) -> String {
        slack_toml_entry_with_spec(token, SLACK_MCP_INSTALL_SPEC)
    }

    fn slack_round_trip_snapshots(
        entries: &HashMap<BaseCodingAgent, Value>,
    ) -> Vec<NativeProfileSnapshot> {
        [
            BaseCodingAgent::Codex,
            BaseCodingAgent::ClaudeCode,
            BaseCodingAgent::Gemini,
            BaseCodingAgent::Grok,
        ]
        .into_iter()
        .map(|executor| {
            let entry = entries.get(&executor).expect("executor entry").clone();
            if matches!(executor, BaseCodingAgent::Codex | BaseCodingAgent::Grok) {
                NativeProfileSnapshot {
                    profile: SharedMcpProfile {
                        executor,
                        display_name: executor.to_string(),
                        supports_mcp: true,
                        config_path: Some(format!("/tmp/{executor}.toml")),
                        servers_path: vec!["mcp_servers".to_string()],
                        read_error: None,
                    },
                    config_path: Some(PathBuf::from(format!("/tmp/{executor}.toml"))),
                    mcp_config: McpConfig::new(
                        vec!["mcp_servers".to_string()],
                        json!({"mcp_servers": {}}),
                        json!({}),
                        true,
                    ),
                    servers: HashMap::from([("slack".to_string(), entry)]),
                }
            } else {
                json_snapshot(executor, "slack", entry)
            }
        })
        .collect()
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
    fn migrates_the_known_legacy_slack_template_while_reconciling_profiles() {
        let legacy = slack_json_entry_with_spec("xoxp-test", "slack-mcp-server@latest");
        let response = reconcile_snapshots(vec![
            toml_snapshot(
                BaseCodingAgent::Codex,
                "slack",
                &slack_toml_entry("xoxp-test"),
            ),
            json_snapshot(BaseCodingAgent::ClaudeCode, "slack", legacy.clone()),
            json_snapshot(BaseCodingAgent::Gemini, "slack", legacy.clone()),
            toml_snapshot(
                BaseCodingAgent::Grok,
                "slack",
                &slack_toml_entry_with_spec("xoxp-test", "slack-mcp-server@latest"),
            ),
        ]);

        assert_eq!(response.conflicts.len(), 0);
        assert_eq!(response.servers.len(), 1);
        let server = &response.servers[0];
        assert_eq!(server.name, "slack");
        assert!(matches!(
            server.source_kind,
            SharedMcpSourceKind::Reconciled
        ));
        assert_eq!(server.assignments.len(), 4);
        assert_eq!(server.native_sources.len(), 4);

        let first_fingerprint = server.native_sources[0]
            .normalized_fingerprint
            .as_ref()
            .expect("fingerprint");
        assert!(
            server
                .native_sources
                .iter()
                .all(|source| source.normalized_fingerprint.as_ref() == Some(first_fingerprint))
        );
        assert_eq!(server.definition.transport, McpTransportKind::Stdio);
        assert_eq!(server.definition.value, slack_json_entry("xoxp-test"));
    }

    #[test]
    fn only_the_exact_legacy_slack_template_is_migrated() {
        let legacy_with_extra_env = json!({
            "command": "npx",
            "args": ["-y", "slack-mcp-server@latest", "--transport", "stdio"],
            "env": {
                "SLACK_MCP_XOXP_TOKEN": "xoxp-test",
                "EXTRA": "value"
            }
        });
        let response = reconcile_snapshots(vec![
            json_snapshot(
                BaseCodingAgent::Codex,
                "slack",
                slack_json_entry("xoxp-test"),
            ),
            json_snapshot(BaseCodingAgent::ClaudeCode, "slack", legacy_with_extra_env),
        ]);

        assert_eq!(response.servers.len(), 0);
        assert_eq!(response.conflicts.len(), 1);
    }

    #[test]
    fn equivalent_slack_conflicts_on_semantic_stdio_differences() {
        let cases = [
            (
                "command",
                json!({
                    "command": "node",
                    "args": ["-y", SLACK_MCP_INSTALL_SPEC, "--transport", "stdio"],
                    "env": {"SLACK_MCP_XOXP_TOKEN": "xoxp-test"}
                }),
            ),
            (
                "arg",
                json!({
                    "command": "npx",
                    "args": ["--yes", SLACK_MCP_INSTALL_SPEC, "--transport", "stdio"],
                    "env": {"SLACK_MCP_XOXP_TOKEN": "xoxp-test"}
                }),
            ),
            (
                "transport",
                json!({
                    "command": "npx",
                    "args": ["-y", SLACK_MCP_INSTALL_SPEC, "--transport", "sse"],
                    "env": {"SLACK_MCP_XOXP_TOKEN": "xoxp-test"}
                }),
            ),
            (
                "release artifact",
                json!({
                    "command": "npx",
                    "args": [
                        "-y",
                        "https://github.com/davidvasandani/slack-mcp-server/releases/download/v1.3.0-vk.1/slack-mcp-server-vk-1.3.0-vk.1.tgz",
                        "--transport",
                        "stdio"
                    ],
                    "env": {"SLACK_MCP_XOXP_TOKEN": "xoxp-test"}
                }),
            ),
            (
                "env key",
                json!({
                    "command": "npx",
                    "args": ["-y", SLACK_MCP_INSTALL_SPEC, "--transport", "stdio"],
                    "env": {"SLACK_TOKEN": "xoxp-test"}
                }),
            ),
            (
                "token value",
                json!({
                    "command": "npx",
                    "args": ["-y", SLACK_MCP_INSTALL_SPEC, "--transport", "stdio"],
                    "env": {"SLACK_MCP_XOXP_TOKEN": "xoxp-other"}
                }),
            ),
        ];

        for (field, changed_entry) in cases {
            let response = reconcile_snapshots(vec![
                toml_snapshot(
                    BaseCodingAgent::Codex,
                    "slack",
                    &slack_toml_entry("xoxp-test"),
                ),
                json_snapshot(BaseCodingAgent::ClaudeCode, "slack", changed_entry),
            ]);

            assert_eq!(
                response.servers.len(),
                0,
                "changed {field} should not reconcile"
            );
            assert_eq!(
                response.conflicts.len(),
                1,
                "changed {field} should conflict"
            );
            assert_eq!(response.conflicts[0].variants.len(), 2);
        }
    }

    #[test]
    fn migrated_slack_definition_materializes_and_reconciles_without_conflict() {
        let legacy = slack_json_entry_with_spec("xoxp-test", "slack-mcp-server@latest");
        let first = reconcile_snapshots(vec![
            json_snapshot(
                BaseCodingAgent::Codex,
                "slack",
                slack_json_entry("xoxp-test"),
            ),
            json_snapshot(BaseCodingAgent::ClaudeCode, "slack", legacy.clone()),
            json_snapshot(BaseCodingAgent::Gemini, "slack", legacy.clone()),
            toml_snapshot(
                BaseCodingAgent::Grok,
                "slack",
                &slack_toml_entry_with_spec("xoxp-test", "slack-mcp-server@latest"),
            ),
        ]);
        let definition = first.servers[0].definition.clone();
        let request = SharedMcpWriteRequest {
            servers: vec![SharedMcpServerInput {
                name: "slack".to_string(),
                display_name: None,
                definition,
                assignments: vec![
                    BaseCodingAgent::Codex,
                    BaseCodingAgent::ClaudeCode,
                    BaseCodingAgent::Gemini,
                    BaseCodingAgent::Grok,
                ],
                native_overrides: HashMap::new(),
            }],
            removed_servers: Vec::new(),
            resolved_conflicts: Vec::new(),
        };
        let entries = request.servers[0]
            .assignments
            .iter()
            .copied()
            .map(|executor| {
                let (servers, _) =
                    plan_servers_for_executor(executor, &HashMap::new(), &request).unwrap();
                let entry = servers.get("slack").expect("slack entry").clone();
                assert_eq!(entry, slack_json_entry("xoxp-test"));
                (executor, entry)
            })
            .collect();

        let second = reconcile_snapshots(slack_round_trip_snapshots(&entries));
        assert_eq!(second.conflicts.len(), 0);
        assert_eq!(second.servers.len(), 1);
        assert_eq!(second.servers[0].assignments.len(), 4);
        assert_eq!(
            second.servers[0].definition.value,
            slack_json_entry("xoxp-test")
        );
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
    fn supports_streamable_http_assignments_for_codex() {
        let definition =
            canonical_definition(&json!({"type":"http","url":"https://example.test/mcp"}));
        assert!(incompatibility_reason(BaseCodingAgent::Codex, &definition).is_none());
        assert_eq!(
            materialize_definition(BaseCodingAgent::Codex, &definition, None).unwrap(),
            json!({"url":"https://example.test/mcp"})
        );
    }

    #[test]
    fn maps_codex_http_headers_to_native_toml_shape() {
        let definition = canonical_definition(&json!({
            "url":"https://example.test/mcp",
            "http_headers":{"Authorization":"Bearer token"}
        }));
        assert_eq!(
            materialize_definition(BaseCodingAgent::Codex, &definition, None).unwrap(),
            json!({
                "url":"https://example.test/mcp",
                "http_headers":{"Authorization":"Bearer token"}
            })
        );
    }

    #[test]
    fn maps_grok_http_to_native_toml_shape_and_rejects_sse() {
        let http = canonical_definition(&json!({
            "type":"http",
            "url":"https://example.test/mcp",
            "headers":{"Authorization":"Bearer token"}
        }));
        assert_eq!(
            materialize_definition(BaseCodingAgent::Grok, &http, None).unwrap(),
            json!({
                "url":"https://example.test/mcp",
                "headers":{"Authorization":"Bearer token"}
            })
        );

        let sse = canonical_definition(&json!({
            "type":"sse",
            "url":"https://example.test/sse"
        }));
        assert!(incompatibility_reason(BaseCodingAgent::Grok, &sse).is_some());
    }

    #[test]
    fn preserves_unrelated_native_servers_in_write_plan() {
        let request = SharedMcpWriteRequest {
            servers: vec![SharedMcpServerInput {
                name: "shared".to_string(),
                display_name: None,
                definition: canonical_definition(&json!({"command":"npx"})),
                assignments: vec![BaseCodingAgent::ClaudeCode],
                native_overrides: HashMap::new(),
            }],
            resolved_conflicts: Vec::new(),
            removed_servers: Vec::new(),
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
                display_name: None,
                definition: canonical_definition(&json!({"command":"npx"})),
                assignments: vec![BaseCodingAgent::Gemini],
                native_overrides: HashMap::new(),
            }],
            resolved_conflicts: Vec::new(),
            removed_servers: Vec::new(),
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

    #[test]
    fn gateway_capability_is_redacted_on_read_and_preserved_on_save() {
        let entry = json!({
            "url":"http://127.0.0.1:3334/mcp-gateway/00000000-0000-0000-0000-000000000001",
            "headers":{"Authorization":"Bearer local-secret"}
        });
        let response = reconcile_snapshots(vec![snapshot(
            BaseCodingAgent::ClaudeCode,
            HashMap::from([("tools".to_string(), entry.clone())]),
        )]);
        assert_eq!(
            response.servers[0].auth_mode,
            SharedMcpAuthMode::SharedGateway
        );
        assert_eq!(
            response.servers[0].definition.value["headers"]["Authorization"],
            "Bearer [REDACTED]"
        );
        assert_eq!(
            response.servers[0].native_sources[0].entry["headers"]["Authorization"],
            "Bearer [REDACTED]"
        );
        let request = SharedMcpWriteRequest {
            servers: vec![SharedMcpServerInput {
                name: "tools".to_string(),
                display_name: None,
                definition: response.servers[0].definition.clone(),
                assignments: vec![BaseCodingAgent::ClaudeCode],
                native_overrides: HashMap::new(),
            }],
            resolved_conflicts: Vec::new(),
            removed_servers: Vec::new(),
        };
        let (next, _) = plan_servers_for_executor(
            BaseCodingAgent::ClaudeCode,
            &HashMap::from([("tools".to_string(), entry.clone())]),
            &request,
        )
        .unwrap();
        assert_eq!(
            next["tools"]["headers"]["Authorization"],
            "Bearer local-secret"
        );

        let mut renamed = request;
        renamed.servers[0].name = "renamed-tools".to_string();
        renamed.removed_servers.push("tools".to_string());
        let (next, _) = plan_servers_for_executor(
            BaseCodingAgent::ClaudeCode,
            &HashMap::from([("tools".to_string(), entry)]),
            &renamed,
        )
        .unwrap();
        assert_eq!(
            next["renamed-tools"]["headers"]["Authorization"],
            "Bearer local-secret"
        );
    }

    #[test]
    fn server_identifier_validation_covers_protocol_edge_cases() {
        for valid in ["vibe_kanban", "Vibe-Kanban", "server123"] {
            assert!(is_valid_server_identifier(valid), "{valid}");
        }
        for invalid in ["Vibe Kanban", "vibe.kanban", "工具", "server!"] {
            assert!(!is_valid_server_identifier(invalid), "{invalid}");
        }
        assert_eq!(suggested_server_identifier("Vibe Kanban"), "vibe_kanban");
        assert_eq!(suggested_server_identifier("vibe...kanban"), "vibe_kanban");
        assert_eq!(suggested_server_identifier("工具"), "mcp_server");
        assert_eq!(
            suggested_server_identifier("  Rovo...Cloud!  "),
            "rovo_cloud"
        );
    }

    #[tokio::test]
    async fn display_labels_round_trip_without_entering_native_definitions() {
        let path = std::env::temp_dir().join(format!(
            "vibe-kanban-mcp-labels-{}.json",
            uuid::Uuid::new_v4()
        ));
        let labels = BTreeMap::from([("atlassian_rovo".to_string(), "Atlassian Rovo".to_string())]);
        write_display_labels_to(&path, &labels).await.unwrap();
        assert_eq!(load_display_labels_from(&path).await.unwrap(), labels);

        let mut response = reconcile_snapshots(vec![snapshot(
            BaseCodingAgent::ClaudeCode,
            HashMap::from([(
                "atlassian_rovo".to_string(),
                json!({"url": "https://rovo.example/mcp"}),
            )]),
        )]);
        attach_display_labels(&mut response, &labels);
        assert_eq!(
            response.servers[0].display_name.as_deref(),
            Some("Atlassian Rovo")
        );
        assert!(
            response.servers[0]
                .definition
                .value
                .get("display_name")
                .is_none()
        );
        let _ = tokio::fs::remove_file(path).await;
    }

    #[test]
    fn display_label_merge_preserves_unresolved_existing_servers() {
        let current = BTreeMap::from([
            ("conflicted".to_string(), "Friendly Conflict".to_string()),
            ("removed".to_string(), "Removed".to_string()),
        ]);
        let request = SharedMcpWriteRequest {
            servers: vec![SharedMcpServerInput {
                name: "updated".to_string(),
                display_name: Some("Updated Label".to_string()),
                definition: canonical_definition(&json!({"command":"npx"})),
                assignments: vec![BaseCodingAgent::ClaudeCode],
                native_overrides: HashMap::new(),
            }],
            removed_servers: vec!["removed".to_string()],
            resolved_conflicts: Vec::new(),
        };
        let existing = HashSet::from(["conflicted".to_string(), "updated".to_string()]);
        assert_eq!(
            merged_display_labels(current, &request, &existing, None),
            BTreeMap::from([
                ("conflicted".to_string(), "Friendly Conflict".to_string()),
                ("updated".to_string(), "Updated Label".to_string()),
            ])
        );
    }

    #[test]
    fn duplicate_identifiers_are_rejected_after_the_user_repairs_them() {
        let server = |name: &str| SharedMcpServerInput {
            name: name.to_string(),
            display_name: None,
            definition: canonical_definition(&json!({"command":"npx"})),
            assignments: vec![BaseCodingAgent::ClaudeCode],
            native_overrides: HashMap::new(),
        };
        let request = SharedMcpWriteRequest {
            servers: vec![server("vibe_kanban"), server("vibe_kanban")],
            removed_servers: Vec::new(),
            resolved_conflicts: Vec::new(),
        };
        assert_eq!(
            validate_write_request(&request).unwrap_err(),
            "MCP server `vibe_kanban` is duplicated"
        );
    }

    #[test]
    fn invalid_identifier_error_includes_a_repair_action() {
        let request = SharedMcpWriteRequest {
            servers: vec![SharedMcpServerInput {
                name: "Vibe Kanban".to_string(),
                display_name: Some("Vibe Kanban".to_string()),
                definition: canonical_definition(&json!({"command":"npx"})),
                assignments: vec![BaseCodingAgent::ClaudeCode],
                native_overrides: HashMap::new(),
            }],
            removed_servers: Vec::new(),
            resolved_conflicts: Vec::new(),
        };
        let error = validate_write_request(&request).unwrap_err();
        assert!(error.contains("^[a-zA-Z0-9_-]+$"));
        assert!(error.contains("Use `vibe_kanban`"));
    }
}
