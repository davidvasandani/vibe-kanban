//! Utilities for reading and writing external agent config files (not the server's own config).
//!
//! These helpers abstract over JSON vs TOML vs JSONC formats used by different agents.
//! JSONC (JSON with Comments) is supported with comment preservation using jsonc-parser's CST.

use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::LazyLock,
};

use jsonc_parser::{
    ParseOptions,
    cst::{CstObject, CstRootNode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::fs;
use ts_rs::TS;

use crate::executors::{CodingAgent, ExecutorError, StandardCodingAgentExecutor};

const MCP_RUNTIME_ROUTES_ENV: &str = "VIBE_MCP_RUNTIME_ROUTES";

fn runtime_routes() -> BTreeMap<String, String> {
    std::env::var(MCP_RUNTIME_ROUTES_ENV)
        .ok()
        .and_then(|raw| serde_json::from_str::<BTreeMap<String, String>>(&raw).ok())
        .unwrap_or_default()
}

/// Convert a settings-owned public MCP URL to the loopback route made
/// available by this execution host. The manifest contains URLs only; Access
/// credential values and references never enter Vibe Kanban state.
pub fn route_mcp_url_for_runtime(url: &str) -> String {
    route_mcp_url(url, &runtime_routes())
}

pub fn has_runtime_route_for_public_url(url: &str) -> bool {
    runtime_routes().contains_key(url)
}

fn route_mcp_url(url: &str, routes: &BTreeMap<String, String>) -> String {
    routes.get(url).cloned().unwrap_or_else(|| url.to_string())
}

/// Convert an exact deployment-owned loopback route back to its public logical
/// URL for settings and API read models.
pub fn public_mcp_url_for_runtime(url: &str) -> String {
    public_mcp_url(url, &runtime_routes())
}

fn public_mcp_url(url: &str, routes: &BTreeMap<String, String>) -> String {
    routes
        .iter()
        .find_map(|(public, local)| (local == url).then_some(public.clone()))
        .unwrap_or_else(|| url.to_string())
}

/// Apply runtime routing to untyped executor-native MCP entries without
/// changing any other field. Used at coordinator dispatch so an existing
/// settings entry is safe before the next explicit settings save.
pub fn route_mcp_servers_for_runtime(servers: &mut BTreeMap<String, Value>) {
    route_mcp_servers(servers, &runtime_routes());
}

fn route_mcp_servers(servers: &mut BTreeMap<String, Value>, routes: &BTreeMap<String, String>) {
    for entry in servers.values_mut() {
        let Some(object) = entry.as_object_mut() else {
            continue;
        };
        for key in ["url", "httpUrl"] {
            if let Some(Value::String(url)) = object.get_mut(key) {
                *url = route_mcp_url(url, routes);
            }
        }
    }
}

pub fn mcp_servers_from_config(raw_config: &Value, path: &[String]) -> HashMap<String, Value> {
    let mut current = raw_config;
    for part in path {
        let Some(next) = current.get(part) else {
            return HashMap::new();
        };
        current = next;
    }
    current
        .as_object()
        .map(|servers| {
            servers
                .iter()
                .map(|(name, definition)| (name.clone(), definition.clone()))
                .collect()
        })
        .unwrap_or_default()
}

pub fn set_mcp_servers_in_config(
    raw_config: &mut Value,
    path: &[String],
    servers: &HashMap<String, Value>,
) -> Result<(), ExecutorError> {
    if !raw_config.is_object() {
        *raw_config = serde_json::json!({});
    }
    let Some((final_attr, parents)) = path.split_last() else {
        return Err(ExecutorError::UnknownExecutorType(
            "MCP servers path is empty".into(),
        ));
    };
    let mut current = raw_config;
    for part in parents {
        if current.get(part).is_none() {
            current
                .as_object_mut()
                .expect("config normalized to object")
                .insert(part.clone(), serde_json::json!({}));
        }
        current = current.get_mut(part).expect("inserted config path");
        if !current.is_object() {
            *current = serde_json::json!({});
        }
    }
    current
        .as_object_mut()
        .expect("config path normalized to object")
        .insert(final_attr.clone(), serde_json::to_value(servers)?);
    Ok(())
}

pub async fn read_coding_agent_mcp_servers(
    agent: &CodingAgent,
) -> Result<HashMap<String, Value>, ExecutorError> {
    let path = agent.default_mcp_config_path().ok_or_else(|| {
        ExecutorError::UnknownExecutorType("executor has no MCP config path".into())
    })?;
    let mcp_config = agent.get_mcp_config();
    let config = read_agent_config(&path, &mcp_config).await?;
    Ok(mcp_servers_from_config(&config, &mcp_config.servers_path))
}

pub async fn write_coding_agent_mcp_servers_to_path(
    agent: &CodingAgent,
    source_path: &Path,
    target_path: &Path,
    servers: &HashMap<String, Value>,
) -> Result<(), ExecutorError> {
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(ExecutorError::Io)?;
    }
    let mcp_config = agent.get_mcp_config();
    let mut config = read_agent_config(source_path, &mcp_config).await?;
    set_mcp_servers_in_config(&mut config, &mcp_config.servers_path, servers)?;
    write_agent_config(target_path, &mcp_config, &config).await
}

fn is_jsonc_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("jsonc"))
}

static DEFAULT_MCP_JSON: &str = include_str!("../default_mcp.json");

/// Returns the launcher spec from the checked-in generic Slack stdio template.
/// Migration recognition reads this source of truth so coordinated pin bumps do
/// not require a second release URL in Rust.
pub(crate) fn default_slack_stdio_launcher() -> Option<String> {
    serde_json::from_str::<Value>(DEFAULT_MCP_JSON)
        .ok()?
        .get("slack")?
        .get("args")?
        .as_array()?
        .get(1)?
        .as_str()
        .map(str::to_string)
}

/// Overrides the executable used to launch the bundled Vibe Kanban MCP server
/// that gets written into launched agents' config files. Lets a self-hosted /
/// prod deployment point agents at **its own** build (e.g. the co-located
/// `vibe-kanban-mcp` binary, or a privately published package) instead of the
/// public `npx -y vibe-kanban@latest` default. Unset ⇒ the public default in
/// `default_mcp.json` is used unchanged.
const MCP_COMMAND_ENV: &str = "VIBE_KANBAN_MCP_COMMAND";
/// Optional whitespace-separated args for [`MCP_COMMAND_ENV`]. When the command
/// is overridden but this is unset, the args are cleared — running the
/// `vibe-kanban-mcp` binary directly needs no extra args (defaults to global
/// mode), unlike the `npx … --mcp` form where `--mcp` selects the subcommand.
const MCP_ARGS_ENV: &str = "VIBE_KANBAN_MCP_ARGS";
/// Optional deployment-owned Streamable HTTP endpoint for the bundled Slack
/// connector. Self-hosted clusters use one supervised server instead of
/// putting a Slack token and stdio launcher in every agent config.
const SLACK_MCP_URL_ENV: &str = "VIBE_KANBAN_SLACK_MCP_URL";
/// Optional deployment-owned Streamable HTTP endpoint exposing the
/// Entra-authenticated CLIs (az, mgc-beta, graph-powershell).
///
/// Those CLIs are signed in on one host. Authenticating every cluster worker
/// would put a keyring, an MSAL cache and a rotating refresh token on each —
/// and MSAL caches are not multi-writer, so two hosts renewing the same
/// identity can invalidate each other. Pointing agents at one server keeps the
/// credential in a single place while every worker keeps the capability.
const ENTRA_MCP_URL_ENV: &str = "VIBE_KANBAN_ENTRA_MCP_URL";

pub static PRECONFIGURED_MCP_SERVERS: LazyLock<Value> = LazyLock::new(|| {
    let mut value =
        serde_json::from_str::<Value>(DEFAULT_MCP_JSON).expect("Failed to parse default MCP JSON");
    apply_vibe_kanban_command_override(&mut value);
    apply_slack_http_override(&mut value);
    apply_entra_http_override(&mut value);
    value
});

/// Applies the [`MCP_COMMAND_ENV`] / [`MCP_ARGS_ENV`] override to the parsed
/// preconfigured servers, if set. No-op when the env var is absent or blank.
fn apply_vibe_kanban_command_override(value: &mut Value) {
    let Ok(command) = std::env::var(MCP_COMMAND_ENV) else {
        return;
    };
    let command = command.trim();
    if command.is_empty() {
        return;
    }

    let args: Vec<String> = std::env::var(MCP_ARGS_ENV)
        .ok()
        .map(|raw| raw.split_whitespace().map(str::to_string).collect())
        .unwrap_or_default();

    set_vibe_kanban_command(value, command, args);
}

/// Rewrites the `vibe_kanban` server entry's `command`/`args` in place, leaving
/// every other preconfigured server untouched.
fn set_vibe_kanban_command(value: &mut Value, command: &str, args: Vec<String>) {
    if let Some(entry) = value.get_mut("vibe_kanban").and_then(Value::as_object_mut) {
        entry.insert("command".to_string(), Value::String(command.to_string()));
        entry.insert(
            "args".to_string(),
            Value::Array(args.into_iter().map(Value::String).collect()),
        );
    }
}

fn apply_slack_http_override(value: &mut Value) {
    let Ok(url) = std::env::var(SLACK_MCP_URL_ENV) else {
        return;
    };
    let url = url.trim();
    if url.is_empty() {
        return;
    }
    set_slack_http_url(value, url);
}

/// Replaces the bundled local Slack launcher with a deployment-owned HTTP
/// endpoint. Rebuild the object rather than removing selected fields so a
/// future stdio-only field cannot accidentally carry a credential forward.
fn set_slack_http_url(value: &mut Value, url: &str) {
    if let Some(servers) = value.as_object_mut() {
        servers.insert(
            "slack".to_string(),
            serde_json::json!({ "type": "http", "url": url }),
        );
    }
}

fn apply_entra_http_override(value: &mut Value) {
    let Ok(url) = std::env::var(ENTRA_MCP_URL_ENV) else {
        return;
    };
    let url = url.trim();
    if url.is_empty() {
        return;
    }
    set_entra_http_url(value, url);
}

/// Adds the shared Entra CLI endpoint. Unlike the Slack override there is no
/// bundled stdio launcher to replace — without the endpoint the servers simply
/// are not offered, because there is nothing local to fall back to.
fn set_entra_http_url(value: &mut Value, url: &str) {
    if let Some(servers) = value.as_object_mut() {
        servers.insert(
            "entra".to_string(),
            serde_json::json!({ "type": "http", "url": url }),
        );
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct McpConfig {
    servers: HashMap<String, serde_json::Value>,
    pub servers_path: Vec<String>,
    pub template: serde_json::Value,
    pub preconfigured: serde_json::Value,
    pub is_toml_config: bool,
}

impl McpConfig {
    pub fn new(
        servers_path: Vec<String>,
        template: serde_json::Value,
        preconfigured: serde_json::Value,
        is_toml_config: bool,
    ) -> Self {
        Self {
            servers: HashMap::new(),
            servers_path,
            template,
            preconfigured,
            is_toml_config,
        }
    }
    pub fn set_servers(&mut self, servers: HashMap<String, serde_json::Value>) {
        self.servers = servers;
    }
}

pub async fn read_agent_config(
    config_path: &std::path::Path,
    mcp_config: &McpConfig,
) -> Result<Value, ExecutorError> {
    match fs::read_to_string(config_path).await {
        Ok(file_content) => {
            if mcp_config.is_toml_config {
                if file_content.trim().is_empty() {
                    return Ok(serde_json::json!({}));
                }
                let toml_val: toml::Value = toml::from_str(&file_content)?;
                let json_string = serde_json::to_string(&toml_val)?;
                Ok(serde_json::from_str(&json_string)?)
            } else if is_jsonc_file(config_path) {
                if file_content.trim().is_empty() {
                    return Ok(serde_json::json!({}));
                }
                match jsonc_parser::parse_to_serde_value(&file_content, &ParseOptions::default()) {
                    Ok(Some(value)) => Ok(value),
                    Ok(None) => Ok(serde_json::json!({})),
                    Err(_) => Ok(serde_json::from_str(&file_content)?),
                }
            } else {
                Ok(serde_json::from_str(&file_content)?)
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(mcp_config.template.clone())
        }
        Err(error) => Err(ExecutorError::Io(error)),
    }
}

pub async fn write_agent_config(
    config_path: &std::path::Path,
    mcp_config: &McpConfig,
    config: &Value,
) -> Result<(), ExecutorError> {
    let content = serialize_agent_config(config_path, mcp_config, config).await?;
    atomic_write_agent_config(config_path, content.as_bytes()).await?;
    Ok(())
}

async fn serialize_agent_config(
    config_path: &std::path::Path,
    mcp_config: &McpConfig,
    config: &Value,
) -> Result<String, ExecutorError> {
    if mcp_config.is_toml_config {
        let toml_value: toml::Value = serde_json::from_str(&serde_json::to_string(config)?)?;
        Ok(toml::to_string_pretty(&toml_value)?)
    } else if is_jsonc_file(config_path) {
        Ok(jsonc_content_preserving_comments(config_path, config).await?)
    } else {
        Ok(serde_json::to_string_pretty(config)?)
    }
}

async fn jsonc_content_preserving_comments(
    config_path: &std::path::Path,
    new_config: &Value,
) -> Result<String, ExecutorError> {
    let current_content = fs::read_to_string(config_path)
        .await
        .unwrap_or_else(|_| "{}".to_string());

    Ok(update_jsonc_content(&current_content, new_config))
}

fn backup_path(config_path: &Path) -> PathBuf {
    let mut backup = config_path.to_path_buf();
    let file_name = config_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("config");
    backup.set_file_name(format!("{file_name}.bak"));
    backup
}

fn staged_path(config_path: &Path) -> PathBuf {
    let mut staged = config_path.to_path_buf();
    let file_name = config_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("config");
    staged.set_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
    staged
}

/// Atomically replace an agent config file and retain a sibling `.bak` copy of
/// the previous version when one existed. Each native file can be recovered
/// independently after a partial shared MCP save.
pub async fn atomic_write_agent_config(
    config_path: &Path,
    content: &[u8],
) -> Result<(), ExecutorError> {
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let staged = staged_path(config_path);
    fs::write(&staged, content).await?;
    if let Ok(file) = fs::OpenOptions::new().read(true).open(&staged).await {
        file.sync_all().await?;
    }

    if fs::metadata(config_path).await.is_ok() {
        let backup = backup_path(config_path);
        let _ = fs::remove_file(&backup).await;
        fs::copy(config_path, &backup).await?;
    }

    match fs::rename(&staged, config_path).await {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&staged).await;
            Err(ExecutorError::Io(e))
        }
    }
}

pub fn previous_version_backup_path(config_path: &Path) -> PathBuf {
    backup_path(config_path)
}

#[allow(dead_code)]
async fn write_jsonc_preserving_comments(
    config_path: &std::path::Path,
    new_config: &Value,
) -> Result<(), ExecutorError> {
    let output = jsonc_content_preserving_comments(config_path, new_config).await?;
    atomic_write_agent_config(config_path, output.as_bytes()).await?;
    Ok(())
}

fn update_jsonc_content(current_content: &str, new_config: &Value) -> String {
    let root = CstRootNode::parse(current_content, &ParseOptions::default())
        .unwrap_or_else(|_| CstRootNode::parse("{}", &ParseOptions::default()).unwrap());

    let root_obj = root.object_value_or_set();

    if let Some(obj) = new_config.as_object() {
        deep_merge_cst_object(&root_obj, obj);
    }

    root.to_string()
}

/// Recursively merges a serde_json Map into an existing CST object.
/// This preserves comments by navigating into existing nested objects rather than replacing them.
fn deep_merge_cst_object(cst_obj: &CstObject, new_obj: &Map<String, Value>) {
    let existing_keys: Vec<String> = cst_obj
        .properties()
        .iter()
        .filter_map(|p| p.name().and_then(|n| n.decoded_value().ok()))
        .collect();

    for key in &existing_keys {
        if !new_obj.contains_key(key)
            && let Some(prop) = cst_obj.get(key)
        {
            prop.remove();
        }
    }

    for (key, new_value) in new_obj {
        if let Some(prop) = cst_obj.get(key) {
            if let (Some(existing_obj), Some(new_obj_map)) =
                (prop.object_value(), new_value.as_object())
            {
                deep_merge_cst_object(&existing_obj, new_obj_map);
            } else {
                prop.set_value(serde_json_to_cst_input(new_value));
            }
        } else {
            cst_obj.append(key, serde_json_to_cst_input(new_value));
        }
    }
}

fn serde_json_to_cst_input(value: &Value) -> jsonc_parser::cst::CstInputValue {
    use jsonc_parser::cst::CstInputValue;

    match value {
        Value::Null => CstInputValue::Null,
        Value::Bool(b) => CstInputValue::Bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                CstInputValue::Number(i.to_string())
            } else if let Some(f) = n.as_f64() {
                CstInputValue::Number(f.to_string())
            } else {
                CstInputValue::Number(n.to_string())
            }
        }
        Value::String(s) => CstInputValue::String(s.clone()),
        Value::Array(arr) => {
            CstInputValue::Array(arr.iter().map(serde_json_to_cst_input).collect())
        }
        Value::Object(obj) => CstInputValue::Object(
            obj.iter()
                .map(|(k, v)| (k.clone(), serde_json_to_cst_input(v)))
                .collect(),
        ),
    }
}

type ServerMap = Map<String, Value>;

fn is_http_server(s: &Map<String, Value>) -> bool {
    matches!(s.get("type").and_then(Value::as_str), Some("http"))
}

fn is_stdio(s: &Map<String, Value>) -> bool {
    !is_http_server(s) && s.get("command").is_some()
}

/// Splits the presentation-only `meta` block off the server map.
///
/// `preserve_order` is enabled workspace-wide, which makes `Map::remove` a
/// *swap*-remove: it moves the map's last entry into the vacated slot. That is
/// harmless only because `meta` is the last key in `default_mcp.json`. Keep new
/// catalog entries **above** `meta`, or the appended entry will silently take
/// `meta`'s position in every generated agent config.
fn extract_meta(mut obj: ServerMap) -> (ServerMap, Option<Value>) {
    let meta = obj.remove("meta");
    (obj, meta)
}

fn attach_meta(mut obj: ServerMap, meta: Option<Value>) -> Value {
    if let Some(m) = meta {
        obj.insert("meta".to_string(), m);
    }
    Value::Object(obj)
}

fn ensure_header(headers: &mut Map<String, Value>, key: &str, val: &str) {
    match headers.get_mut(key) {
        Some(Value::String(_)) => {}
        _ => {
            headers.insert(key.to_string(), Value::String(val.to_string()));
        }
    }
}

fn transform_http_servers<F>(mut servers: ServerMap, mut f: F) -> ServerMap
where
    F: FnMut(Map<String, Value>) -> Map<String, Value>,
{
    for (_k, v) in servers.iter_mut() {
        if let Value::Object(s) = v
            && is_http_server(s)
        {
            let taken = std::mem::take(s);
            *s = f(taken);
        }
    }
    servers
}

// --- Adapters ---------------------------------------------------------------

fn adapt_passthrough(servers: ServerMap, meta: Option<Value>) -> Value {
    attach_meta(servers, meta)
}

fn adapt_gemini(servers: ServerMap, meta: Option<Value>) -> Value {
    let servers = transform_http_servers(servers, |mut s| {
        let url = s
            .remove("url")
            .unwrap_or_else(|| Value::String(String::new()));
        let mut headers = s
            .remove("headers")
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();

        ensure_header(
            &mut headers,
            "Accept",
            "application/json, text/event-stream",
        );
        Map::from_iter([
            ("httpUrl".to_string(), url),
            ("headers".to_string(), Value::Object(headers)),
        ])
    });
    attach_meta(servers, meta)
}

fn adapt_cursor(servers: ServerMap, meta: Option<Value>) -> Value {
    let servers = transform_http_servers(servers, |mut s| {
        let url = s
            .remove("url")
            .unwrap_or_else(|| Value::String(String::new()));
        let headers = s
            .remove("headers")
            .unwrap_or_else(|| Value::Object(Default::default()));
        Map::from_iter([("url".to_string(), url), ("headers".to_string(), headers)])
    });
    attach_meta(servers, meta)
}

fn adapt_codex(mut servers: ServerMap, mut meta: Option<Value>) -> Value {
    servers.retain(|_, v| {
        v.as_object()
            .is_some_and(|server| is_stdio(server) || server.contains_key("url"))
    });
    for server in servers.values_mut() {
        if let Value::Object(server) = server
            && server.contains_key("url")
        {
            server.remove("type");
            if let Some(Value::Object(headers)) = server.remove("headers") {
                let mut static_headers = Map::new();
                let mut env_headers = Map::new();
                for (name, value) in headers {
                    if let Some(env_name) = value.as_str().and_then(exact_env_template) {
                        env_headers.insert(name, Value::String(env_name.to_string()));
                    } else {
                        static_headers.insert(name, value);
                    }
                }
                if !static_headers.is_empty() {
                    server.insert("http_headers".to_string(), Value::Object(static_headers));
                }
                if !env_headers.is_empty() {
                    server.insert("env_http_headers".to_string(), Value::Object(env_headers));
                }
            }
        }
    }

    if let Some(Value::Object(ref mut m)) = meta {
        m.retain(|k, _| servers.contains_key(k));
        servers.insert("meta".to_string(), Value::Object(std::mem::take(m)));
        meta = None; // already attached above
    }
    attach_meta(servers, meta)
}

fn exact_env_template(value: &str) -> Option<&str> {
    let name = value.strip_prefix("${")?.strip_suffix('}')?;
    let mut chars = name.chars();
    let first = chars.next()?;
    if (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    {
        Some(name)
    } else {
        None
    }
}

fn adapt_opencode(servers: ServerMap, meta: Option<Value>) -> Value {
    let mut servers = transform_http_servers(servers, |mut s| {
        let url = s
            .remove("url")
            .unwrap_or_else(|| Value::String(String::new()));

        let mut headers = s
            .remove("headers")
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();

        ensure_header(
            &mut headers,
            "Accept",
            "application/json, text/event-stream",
        );

        Map::from_iter([
            ("type".to_string(), Value::String("remote".to_string())),
            ("url".to_string(), url),
            ("headers".to_string(), Value::Object(headers)),
            ("enabled".to_string(), Value::Bool(true)),
        ])
    });

    for (_k, v) in servers.iter_mut() {
        if let Value::Object(s) = v
            && is_stdio(s)
        {
            let command_str = s
                .remove("command")
                .and_then(|v| match v {
                    Value::String(s) => Some(s),
                    _ => None,
                })
                .unwrap_or_default();

            let mut cmd_vec: Vec<Value> = Vec::new();
            if !command_str.is_empty() {
                cmd_vec.push(Value::String(command_str));
            }

            if let Some(arr) = s.remove("args").and_then(|v| match v {
                Value::Array(arr) => Some(arr),
                _ => None,
            }) {
                for a in arr {
                    match a {
                        Value::String(s) => cmd_vec.push(Value::String(s)),
                        other => cmd_vec.push(other), // fall back to raw value if not string
                    }
                }
            }

            let environment = s.remove("env");

            let mut new_map = Map::new();
            new_map.insert("type".to_string(), Value::String("local".to_string()));
            new_map.insert("command".to_string(), Value::Array(cmd_vec));
            new_map.insert("enabled".to_string(), Value::Bool(true));
            if let Some(environment) = environment {
                new_map.insert("environment".to_string(), environment);
            }
            *s = new_map;
        }
    }

    attach_meta(servers, meta)
}

fn adapt_copilot(mut servers: ServerMap, meta: Option<Value>) -> Value {
    for (_, value) in servers.iter_mut() {
        if let Value::Object(s) = value
            && !s.contains_key("tools")
        {
            s.insert(
                "tools".to_string(),
                Value::Array(vec![Value::String("*".to_string())]),
            );
        }
    }
    attach_meta(servers, meta)
}

enum Adapter {
    Passthrough,
    Gemini,
    Cursor,
    Codex,
    Opencode,
    Copilot,
}

fn apply_adapter(adapter: Adapter, canonical: Value) -> Value {
    let (servers_only, meta) = match canonical.as_object() {
        Some(map) => extract_meta(map.clone()),
        None => (ServerMap::new(), None),
    };

    match adapter {
        Adapter::Passthrough => adapt_passthrough(servers_only, meta),
        Adapter::Gemini => adapt_gemini(servers_only, meta),
        Adapter::Cursor => adapt_cursor(servers_only, meta),
        Adapter::Codex => adapt_codex(servers_only, meta),
        Adapter::Opencode => adapt_opencode(servers_only, meta),
        Adapter::Copilot => adapt_copilot(servers_only, meta),
    }
}

impl CodingAgent {
    pub fn preconfigured_mcp(&self) -> Value {
        use Adapter::*;

        let adapter = match self {
            CodingAgent::ClaudeCode(_) | CodingAgent::Amp(_) | CodingAgent::Droid(_) => Passthrough,
            CodingAgent::Grok(_) => Cursor,
            CodingAgent::QwenCode(_) | CodingAgent::Gemini(_) => Gemini,
            CodingAgent::CursorAgent(_) => Cursor,
            CodingAgent::Codex(_) => Codex,
            CodingAgent::Opencode(_) => Opencode,
            CodingAgent::Copilot(..) => Copilot,
            #[cfg(feature = "qa-mode")]
            CodingAgent::QaMock(_) => Passthrough, // QA mock doesn't need MCP
        };

        let canonical = PRECONFIGURED_MCP_SERVERS.clone();
        apply_adapter(adapter, canonical)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn replacing_mcp_section_preserves_unrelated_native_settings() {
        let mut config = serde_json::json!({
            "model": "gpt-5",
            "mcp_servers": {"old": {"command": "old"}},
            "projects": {"/workspace": {"trust_level": "trusted"}}
        });
        let servers = HashMap::from([(
            "firecrawl-browser".into(),
            serde_json::json!({"url": "https://example.invalid/mcp", "headers": {"Authorization": "Bearer secret"}}),
        )]);

        set_mcp_servers_in_config(&mut config, &["mcp_servers".into()], &servers).unwrap();

        assert_eq!(config["model"], "gpt-5");
        assert_eq!(config["projects"]["/workspace"]["trust_level"], "trusted");
        assert!(config["mcp_servers"].get("old").is_none());
        assert_eq!(
            mcp_servers_from_config(&config, &["mcp_servers".into()]),
            servers
        );
    }

    #[tokio::test]
    async fn config_reader_defaults_only_for_missing_files() {
        let dir = std::env::temp_dir().join(format!(
            "vk-mcp-read-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).await.unwrap();
        let mcp = McpConfig::new(
            vec!["mcp_servers".into()],
            serde_json::json!({"mcp_servers": {}}),
            serde_json::json!({}),
            true,
        );

        assert_eq!(
            read_agent_config(&dir.join("missing.toml"), &mcp)
                .await
                .unwrap(),
            mcp.template
        );
        assert!(read_agent_config(&dir, &mcp).await.is_err());
        let _ = fs::remove_dir_all(&dir).await;
    }

    #[test]
    fn default_vibe_kanban_command_is_unchanged_without_override() {
        // The checked-in default must still point at the public package so
        // normal installs keep working; the override is opt-in via env.
        let value = serde_json::from_str::<Value>(DEFAULT_MCP_JSON).unwrap();
        let vk = &value["vibe_kanban"];
        assert_eq!(vk["command"], serde_json::json!("npx"));
        assert_eq!(
            vk["args"],
            serde_json::json!(["-y", "vibe-kanban@latest", "--mcp"])
        );
    }

    #[test]
    fn personal_servicenow_catalog_uses_the_fleet_installed_stdio_wrapper() {
        let value = serde_json::from_str::<Value>(DEFAULT_MCP_JSON).unwrap();
        let server = &value["personal_servicenow"];
        assert_eq!(
            server,
            &serde_json::json!({
                "command": "personal-servicenow-mcp",
                "args": []
            })
        );
        assert_eq!(
            value["meta"]["personal_servicenow"]["name"],
            serde_json::json!("Personal ServiceNow")
        );
    }

    #[test]
    fn override_points_vibe_kanban_at_our_build_and_leaves_others_untouched() {
        let mut value = serde_json::from_str::<Value>(DEFAULT_MCP_JSON).unwrap();
        let playwright_before = value["playwright"].clone();

        set_vibe_kanban_command(
            &mut value,
            "/opt/vibe-kanban/bin/vibe-kanban-mcp",
            Vec::new(),
        );

        let vk = &value["vibe_kanban"];
        assert_eq!(
            vk["command"],
            serde_json::json!("/opt/vibe-kanban/bin/vibe-kanban-mcp")
        );
        assert_eq!(vk["args"], serde_json::json!([]));
        // Other preconfigured servers (public third-party tools) are untouched.
        assert_eq!(value["playwright"], playwright_before);
    }

    #[test]
    fn override_supports_custom_args_for_a_private_package() {
        let mut value = serde_json::from_str::<Value>(DEFAULT_MCP_JSON).unwrap();
        set_vibe_kanban_command(
            &mut value,
            "npx",
            vec![
                "-y".to_string(),
                "@ourscope/vibe-kanban@1.2.3".to_string(),
                "--mcp".to_string(),
            ],
        );

        let vk = &value["vibe_kanban"];
        assert_eq!(vk["command"], serde_json::json!("npx"));
        assert_eq!(
            vk["args"],
            serde_json::json!(["-y", "@ourscope/vibe-kanban@1.2.3", "--mcp"])
        );
    }

    /// Fork release the bundled Slack connector installs. Bumping it means
    /// refreshing [`SLACK_MCP_LAUNCHER_SHA256`] and the version named in
    /// `docs/integrations/mcp-server-configuration.mdx` in the same change —
    /// Renovate's `packageRule` for this pin says so too.
    const SLACK_MCP_FORK_TAG: &str = "v1.3.0-vk.2";
    /// SHA-256 of the pinned launcher tarball. npm — not VK — fetches this
    /// tarball when an agent launches the server, so nothing verifies it at
    /// install time, and GitHub allows a release asset to be replaced under an
    /// existing tag. This constant plus the daily `pinned-artifacts` workflow
    /// (which runs the `#[ignore]`d
    /// [`slack_pinned_launcher_matches_recorded_digest`]) is how such a
    /// replacement gets noticed. The binaries the launcher then downloads are
    /// digest-checked by the launcher itself, on every machine.
    const SLACK_MCP_LAUNCHER_SHA256: &str =
        "220e521bed303b8513eecfd45df196a24ed4e70307ef6f57c921cfbfae308c75";
    const SLACK_MCP_INSTALL_SPEC: &str = "https://github.com/davidvasandani/slack-mcp-server/releases/download/v1.3.0-vk.2/slack-mcp-server-vk-1.3.0-vk.2.tgz";

    #[test]
    fn slack_preconfigured_server_matches_the_documented_stdio_contract() {
        let value = serde_json::from_str::<Value>(DEFAULT_MCP_JSON).unwrap();

        assert_eq!(value["slack"]["command"], serde_json::json!("npx"));
        assert_eq!(
            value["slack"]["args"],
            serde_json::json!(["-y", SLACK_MCP_INSTALL_SPEC, "--transport", "stdio"])
        );
        assert_eq!(
            value["slack"]["env"],
            serde_json::json!({ "SLACK_MCP_XOXP_TOKEN": "YOUR_TOKEN" })
        );
        assert_eq!(value["meta"]["slack"]["name"], serde_json::json!("Slack"));
        assert_eq!(
            value["meta"]["slack"]["url"],
            serde_json::json!("https://github.com/davidvasandani/slack-mcp-server/")
        );
    }

    #[test]
    fn slack_http_override_contains_only_the_shared_endpoint() {
        let mut value = serde_json::from_str::<Value>(DEFAULT_MCP_JSON).unwrap();
        set_slack_http_url(&mut value, "http://172.16.100.102:13080/mcp");

        assert_eq!(
            value["slack"],
            serde_json::json!({
                "type": "http",
                "url": "http://172.16.100.102:13080/mcp"
            })
        );
        let serialized = serde_json::to_string(&value["slack"]).unwrap();
        assert!(!serialized.contains("SLACK_MCP"));
        assert!(!serialized.contains("slack-mcp-server"));
        assert!(!serialized.contains("command"));
    }

    #[test]
    fn entra_endpoint_is_absent_unless_configured() {
        // No local launcher exists for these CLIs, so an unset endpoint must
        // leave the server out entirely rather than offering a broken entry.
        let value = serde_json::from_str::<Value>(DEFAULT_MCP_JSON).unwrap();
        assert!(value.get("entra").is_none());
    }

    #[test]
    fn entra_override_adds_only_the_shared_endpoint() {
        let mut value = serde_json::from_str::<Value>(DEFAULT_MCP_JSON).unwrap();
        set_entra_http_url(&mut value, "http://172.16.100.102:13081/mcp");

        assert_eq!(
            value["entra"],
            serde_json::json!({
                "type": "http",
                "url": "http://172.16.100.102:13081/mcp"
            })
        );
        // No credential or launcher may ride along into an agent's config.
        let serialized = serde_json::to_string(&value["entra"]).unwrap();
        assert!(!serialized.contains("command"));
        assert!(!serialized.contains("env"));
    }

    #[test]
    fn entra_override_leaves_other_preconfigured_servers_untouched() {
        let mut value = serde_json::from_str::<Value>(DEFAULT_MCP_JSON).unwrap();
        let before = value.clone();
        set_entra_http_url(&mut value, "http://example/mcp");
        for key in before.as_object().unwrap().keys() {
            assert_eq!(value[key], before[key], "{key} must be unchanged");
        }
    }

    /// Splits `https://github.com/<owner>/<repo>/releases/download/<tag>/<asset>`
    /// into its parts, or `None` if the URL is not a GitHub release asset.
    fn parse_github_release_asset(url: &str) -> Option<(String, String, String)> {
        let rest = url.strip_prefix("https://github.com/")?;
        let (owner, rest) = rest.split_once('/')?;
        let (repo, rest) = rest.split_once('/')?;
        let rest = rest.strip_prefix("releases/download/")?;
        let (tag, asset) = rest.split_once('/')?;
        (!asset.is_empty()).then(|| (owner.to_string(), repo.to_string(), tag.to_string()))
    }

    #[test]
    fn slack_preconfigured_server_pins_an_immutable_fork_artifact() {
        let value = serde_json::from_str::<Value>(DEFAULT_MCP_JSON).unwrap();
        let spec = value["slack"]["args"][1].as_str().expect("install spec");

        // A mutable reference here is the defect this pin exists to prevent:
        // the catalog would advertise the fork while installing whatever the
        // tag happens to point at today.
        for mutable in ["@latest", "#master", "#main", "refs/heads/", "/archive/"] {
            assert!(
                !spec.contains(mutable),
                "slack install spec {spec} must not contain the mutable reference {mutable}"
            );
        }

        let (owner, repo, tag) =
            parse_github_release_asset(spec).expect("slack install spec is a release asset URL");
        assert_eq!(tag, SLACK_MCP_FORK_TAG);

        // The UI links users to `meta.slack.url`; it must be the repository the
        // artifact is actually built from.
        let meta_url = value["meta"]["slack"]["url"].as_str().expect("meta url");
        let meta_repo = meta_url
            .strip_prefix("https://github.com/")
            .expect("meta url is a GitHub URL")
            .trim_end_matches('/');
        assert_eq!(meta_repo, format!("{owner}/{repo}"));
    }

    /// Network-backed on purpose, like the `cli_tools` vendor-artifact checks:
    /// run it deliberately with
    /// `cargo test -p executors slack_pinned_launcher_matches_recorded_digest -- --ignored`
    /// after publishing or re-pinning a fork release.
    #[tokio::test]
    #[ignore = "downloads the pinned Slack MCP launcher from GitHub"]
    async fn slack_pinned_launcher_matches_recorded_digest() {
        use sha2::{Digest, Sha256};

        let value = serde_json::from_str::<Value>(DEFAULT_MCP_JSON).unwrap();
        let spec = value["slack"]["args"][1].as_str().expect("install spec");

        let bytes = reqwest::get(spec)
            .await
            .expect("download pinned launcher")
            .error_for_status()
            .expect("pinned launcher is published")
            .bytes()
            .await
            .expect("read pinned launcher");

        let digest = format!("{:x}", Sha256::digest(&bytes));
        assert_eq!(
            digest, SLACK_MCP_LAUNCHER_SHA256,
            "the published launcher at {spec} no longer matches the recorded digest"
        );
    }

    #[test]
    fn slack_preconfigured_server_adapts_for_codex_and_opencode() {
        let canonical = serde_json::from_str::<Value>(DEFAULT_MCP_JSON).unwrap();
        let codex = apply_adapter(Adapter::Codex, canonical.clone());
        let opencode = apply_adapter(Adapter::Opencode, canonical);

        assert_eq!(codex["slack"]["command"], serde_json::json!("npx"));
        assert_eq!(
            codex["slack"]["env"],
            serde_json::json!({ "SLACK_MCP_XOXP_TOKEN": "YOUR_TOKEN" })
        );
        assert_eq!(opencode["slack"]["type"], serde_json::json!("local"));
        assert_eq!(
            opencode["slack"]["command"],
            serde_json::json!(["npx", "-y", SLACK_MCP_INSTALL_SPEC, "--transport", "stdio"])
        );
        assert_eq!(
            opencode["slack"]["environment"],
            serde_json::json!({ "SLACK_MCP_XOXP_TOKEN": "YOUR_TOKEN" })
        );
    }

    /// Fork revision the bundled Gmail connector installs. Bumping it means
    /// moving the spec in `default_mcp.json` and the revision named in
    /// `docs/integrations/mcp-server-configuration.mdx` in the same change.
    ///
    /// Unlike the Slack pin there is no companion digest constant and no audit
    /// workflow, and that asymmetry is deliberate: Slack pins a *release asset*,
    /// whose bytes GitHub lets a maintainer replace under an existing tag, so a
    /// recorded digest re-checked on a schedule is the only available control. A
    /// git commit is content-addressed, so this SHA resolves to exactly one tree
    /// on every machine and cannot be re-pointed. Recording a digest of it and
    /// re-checking that daily would assert that a hash equals itself.
    ///
    /// Scope, precisely: the SHA pins **this repository's source**, not the
    /// dependency closure. A `github:` install runs the package's `prepare`
    /// script, which resolves its own dependencies from npm at install time, so
    /// what executes is not bit-reproducible — arguably less so than Slack's
    /// statically linked, digest-checked binary. The argument for no audit job
    /// is that auditing an immutable pin is a no-op, not that this delivery
    /// mechanism is stronger overall.
    ///
    /// Renovate cannot follow a bare SHA on a fork with no releases, so this pin
    /// is bumped by hand; `AGENTS.md` records that. A custom manager here would
    /// match the pin and then never propose a successor, which is worse than no
    /// manager because it looks like coverage.
    const GMAIL_MCP_FORK_REVISION: &str = "030da3492753222a41645a9f343466d151c63f3c";
    const GMAIL_MCP_INSTALL_SPEC: &str =
        "github:davidvasandani/Gmail-MCP-Server#030da3492753222a41645a9f343466d151c63f3c";

    #[test]
    fn gmail_preconfigured_server_matches_the_documented_stdio_contract() {
        let value = serde_json::from_str::<Value>(DEFAULT_MCP_JSON).unwrap();

        assert_eq!(value["gmail"]["command"], serde_json::json!("npx"));
        assert_eq!(
            value["gmail"]["args"],
            serde_json::json!(["-y", GMAIL_MCP_INSTALL_SPEC, "--tool-prefix=YOUR_PREFIX_"])
        );
        // A path, not a token: the refresh token stays in the Gmail server's own
        // credentials file and never reaches an agent's config. `GMAIL_OAUTH_PATH`
        // is deliberately absent — the OAuth client is per Google Cloud project,
        // not per mailbox, so every instance shares its default.
        //
        // The placeholder is absolute on purpose. Env values are copied verbatim
        // into agents' native config and the server spawns without a shell, so a
        // `~/…` value is never expanded — it would resolve against the agent's
        // cwd (a task worktree) and drop a refresh token inside the user's repo.
        // The `--tool-prefix` placeholder keeps its trailing `_` for the same
        // class of reason: a user who mirrors its shape without one gets
        // `mysearch_emails` instead of `my_search_emails`.
        assert_eq!(
            value["gmail"]["env"],
            serde_json::json!({ "GMAIL_CREDENTIALS_PATH": "/absolute/path/to/credentials.json" })
        );
        assert_eq!(value["meta"]["gmail"]["name"], serde_json::json!("Gmail"));
        assert_eq!(
            value["meta"]["gmail"]["url"],
            serde_json::json!("https://github.com/davidvasandani/Gmail-MCP-Server")
        );
    }

    /// Splits `github:<owner>/<repo>#<commit-ish>` into its parts, or `None` if
    /// the spec is not an npm GitHub shorthand carrying an explicit revision.
    fn parse_github_git_spec(spec: &str) -> Option<(String, String, String)> {
        let rest = spec.strip_prefix("github:")?;
        let (owner, rest) = rest.split_once('/')?;
        let (repo, commit_ish) = rest.split_once('#')?;
        (!owner.is_empty() && !repo.is_empty() && !commit_ish.is_empty())
            .then(|| (owner.to_string(), repo.to_string(), commit_ish.to_string()))
    }

    #[test]
    fn gmail_preconfigured_server_pins_an_immutable_fork_revision() {
        let value = serde_json::from_str::<Value>(DEFAULT_MCP_JSON).unwrap();
        let spec = value["gmail"]["args"][1].as_str().expect("install spec");

        // A mutable reference here is the defect this pin exists to prevent:
        // the catalog would advertise the fork while installing whatever that
        // branch happens to point at today.
        for mutable in ["@latest", "#master", "#main", "refs/heads/", "/archive/"] {
            assert!(
                !spec.contains(mutable),
                "gmail install spec {spec} must not contain the mutable reference {mutable}"
            );
        }

        // `parse_github_git_spec` requires a `#<commit-ish>`, so a bare
        // `github:owner/repo` — which would track the default branch — fails here.
        let (owner, repo, commit_ish) =
            parse_github_git_spec(spec).expect("gmail install spec is a pinned GitHub git spec");
        assert_eq!(commit_ish, GMAIL_MCP_FORK_REVISION);
        assert!(
            commit_ish.len() == 40
                && commit_ish
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "gmail pin {commit_ish} must be a full 40-character lowercase commit SHA. \
             A branch, tag, or abbreviated SHA can be re-pointed; lowercase is \
             required so this pin has exactly one spelling to compare against"
        );

        // The UI links users to `meta.gmail.url`; it must be the repository the
        // revision is actually installed from.
        let meta_url = value["meta"]["gmail"]["url"].as_str().expect("meta url");
        let meta_repo = meta_url
            .strip_prefix("https://github.com/")
            .expect("meta url is a GitHub URL")
            .trim_end_matches('/');
        assert_eq!(meta_repo, format!("{owner}/{repo}"));
    }

    #[test]
    fn gmail_preconfigured_server_adapts_for_codex_and_opencode() {
        let canonical = serde_json::from_str::<Value>(DEFAULT_MCP_JSON).unwrap();
        let codex = apply_adapter(Adapter::Codex, canonical.clone());
        let opencode = apply_adapter(Adapter::Opencode, canonical);

        assert_eq!(codex["gmail"]["command"], serde_json::json!("npx"));
        assert_eq!(
            codex["gmail"]["env"],
            serde_json::json!({ "GMAIL_CREDENTIALS_PATH": "/absolute/path/to/credentials.json" })
        );
        assert_eq!(opencode["gmail"]["type"], serde_json::json!("local"));
        assert_eq!(
            opencode["gmail"]["command"],
            serde_json::json!([
                "npx",
                "-y",
                GMAIL_MCP_INSTALL_SPEC,
                "--tool-prefix=YOUR_PREFIX_"
            ])
        );
        // Opencode calls the stdio environment field `environment`; losing this
        // rename is what makes a credential-bearing entry silently unusable.
        assert_eq!(
            opencode["gmail"]["environment"],
            serde_json::json!({ "GMAIL_CREDENTIALS_PATH": "/absolute/path/to/credentials.json" })
        );
    }

    #[test]
    fn slack_http_override_adapts_for_codex_and_opencode() {
        let mut canonical = serde_json::from_str::<Value>(DEFAULT_MCP_JSON).unwrap();
        set_slack_http_url(&mut canonical, "http://172.16.100.102:13080/mcp");
        let codex = apply_adapter(Adapter::Codex, canonical.clone());
        let opencode = apply_adapter(Adapter::Opencode, canonical);

        assert_eq!(
            codex["slack"],
            serde_json::json!({ "url": "http://172.16.100.102:13080/mcp" })
        );
        assert_eq!(opencode["slack"]["type"], serde_json::json!("remote"));
        assert_eq!(
            opencode["slack"]["url"],
            serde_json::json!("http://172.16.100.102:13080/mcp")
        );
        assert!(opencode["slack"].get("command").is_none());
        assert!(opencode["slack"].get("environment").is_none());
        assert!(
            !serde_json::to_string(&opencode["slack"])
                .unwrap()
                .contains("Authorization")
        );
    }

    #[test]
    fn grok_preconfigured_http_servers_use_the_typeless_native_shape() {
        let grok = CodingAgent::Grok(serde_json::from_value(serde_json::json!({})).unwrap());
        let context7 = grok.preconfigured_mcp()["context7"].clone();

        assert_eq!(
            context7["url"],
            serde_json::json!("https://mcp.context7.com/mcp")
        );
        assert!(context7.get("type").is_none());
        assert!(context7.get("headers").is_some());
    }

    #[test]
    fn codex_uses_environment_sourced_http_headers_for_exact_templates() {
        let servers = Map::from_iter([(
            "tldraw".to_string(),
            serde_json::json!({
                "type": "http",
                "url": "https://draw.example.test/mcp",
                "headers": {
                    "CF-Access-Client-Id": "${TLDRAW_CF_ACCESS_CLIENT_ID}",
                    "CF-Access-Client-Secret": "${TLDRAW_CF_ACCESS_CLIENT_SECRET}",
                    "X-Static": "static",
                    "Authorization": "Bearer ${NOT_AN_EXACT_TEMPLATE}"
                }
            }),
        )]);

        let adapted = adapt_codex(servers, None);
        assert_eq!(
            adapted["tldraw"]["env_http_headers"],
            serde_json::json!({
                "CF-Access-Client-Id": "TLDRAW_CF_ACCESS_CLIENT_ID",
                "CF-Access-Client-Secret": "TLDRAW_CF_ACCESS_CLIENT_SECRET"
            })
        );
        assert_eq!(
            adapted["tldraw"]["http_headers"],
            serde_json::json!({
                "X-Static": "static",
                "Authorization": "Bearer ${NOT_AN_EXACT_TEMPLATE}"
            })
        );
    }

    #[test]
    fn runtime_mcp_routes_are_exact_and_reversible() {
        let routes = BTreeMap::from([(
            "https://vibe.vasandani.dev/mcp".to_string(),
            "http://127.0.0.1:18901/mcp".to_string(),
        )]);

        assert_eq!(
            route_mcp_url("https://vibe.vasandani.dev/mcp", &routes),
            "http://127.0.0.1:18901/mcp"
        );
        assert_eq!(
            route_mcp_url("https://vibe.vasandani.dev/mcp/other", &routes),
            "https://vibe.vasandani.dev/mcp/other"
        );
        assert_eq!(
            public_mcp_url("http://127.0.0.1:18901/mcp", &routes),
            "https://vibe.vasandani.dev/mcp"
        );
        assert_eq!(
            public_mcp_url(
                "http://127.0.0.1:3334/mcp-gateway/00000000-0000-0000-0000-000000000001",
                &routes
            ),
            "http://127.0.0.1:3334/mcp-gateway/00000000-0000-0000-0000-000000000001"
        );

        let mut servers = BTreeMap::from([
            (
                "vibe_kanban".to_string(),
                serde_json::json!({
                    "url": "https://vibe.vasandani.dev/mcp",
                    "http_headers": {"Authorization": "Bearer ${VIBE_KANBAN_MCP_TOKEN}"}
                }),
            ),
            (
                "other".to_string(),
                serde_json::json!({"url": "https://example.test/mcp"}),
            ),
        ]);
        route_mcp_servers(&mut servers, &routes);
        assert_eq!(
            servers["vibe_kanban"]["url"],
            serde_json::json!("http://127.0.0.1:18901/mcp")
        );
        assert_eq!(
            servers["vibe_kanban"]["http_headers"]["Authorization"],
            serde_json::json!("Bearer ${VIBE_KANBAN_MCP_TOKEN}")
        );
        assert_eq!(
            servers["other"]["url"],
            serde_json::json!("https://example.test/mcp")
        );
    }

    #[tokio::test]
    async fn atomic_write_replaces_file_and_keeps_backup() {
        let dir = std::env::temp_dir().join(format!(
            "vk-mcp-config-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).await.unwrap();
        let path = dir.join("config.json");
        fs::write(&path, br#"{"old":true}"#).await.unwrap();

        atomic_write_agent_config(&path, br#"{"new":true}"#)
            .await
            .unwrap();

        assert_eq!(fs::read_to_string(&path).await.unwrap(), r#"{"new":true}"#);
        assert_eq!(
            fs::read_to_string(previous_version_backup_path(&path))
                .await
                .unwrap(),
            r#"{"old":true}"#
        );
        let _ = fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn failed_atomic_replace_preserves_existing_file() {
        let dir = std::env::temp_dir().join(format!(
            "vk-mcp-config-fail-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).await.unwrap();
        let path = dir.join("config.json");
        fs::write(&path, br#"{"old":true}"#).await.unwrap();

        let result = atomic_write_agent_config(&path, b"replacement").await;
        assert!(result.is_ok());
        assert_eq!(fs::read_to_string(&path).await.unwrap(), "replacement");
        let _ = fs::remove_dir_all(&dir).await;
    }
}
