//! Utilities for reading and writing external agent config files (not the server's own config).
//!
//! These helpers abstract over JSON vs TOML vs JSONC formats used by different agents.
//! JSONC (JSON with Comments) is supported with comment preservation using jsonc-parser's CST.

use std::{
    collections::HashMap,
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

use crate::executors::{CodingAgent, ExecutorError};

fn is_jsonc_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("jsonc"))
}

static DEFAULT_MCP_JSON: &str = include_str!("../default_mcp.json");

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

pub static PRECONFIGURED_MCP_SERVERS: LazyLock<Value> = LazyLock::new(|| {
    let mut value =
        serde_json::from_str::<Value>(DEFAULT_MCP_JSON).expect("Failed to parse default MCP JSON");
    apply_vibe_kanban_command_override(&mut value);
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
    if let Ok(file_content) = fs::read_to_string(config_path).await {
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
    } else {
        Ok(mcp_config.template.clone())
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
            if let Some(headers) = server.remove("headers") {
                server.insert("http_headers".to_string(), headers);
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

    #[test]
    fn slack_preconfigured_server_matches_the_documented_stdio_contract() {
        let value = serde_json::from_str::<Value>(DEFAULT_MCP_JSON).unwrap();

        assert_eq!(value["slack"]["command"], serde_json::json!("npx"));
        assert_eq!(
            value["slack"]["args"],
            serde_json::json!(["-y", "slack-mcp-server@latest", "--transport", "stdio"])
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
            serde_json::json!([
                "npx",
                "-y",
                "slack-mcp-server@latest",
                "--transport",
                "stdio"
            ])
        );
        assert_eq!(
            opencode["slack"]["environment"],
            serde_json::json!({ "SLACK_MCP_XOXP_TOKEN": "YOUR_TOKEN" })
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
