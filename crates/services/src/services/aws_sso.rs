//! AWS SSO profile management.
//!
//! Vibe Kanban edits the user's AWS config file (`~/.aws/config`) as a guest:
//! only the managed `[profile <name>]` / `[sso-session <name>]` sections are
//! rewritten, everything else is preserved byte-for-byte, writes are atomic
//! (temp file + rename), and a file that cannot be parsed is never rewritten.
//! Credentials are never touched — sign-in runs `aws sso login` in a PTY and
//! tokens stay in the AWS CLI's own storage (`~/.aws/sso/cache`).

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Mutex as StdMutex, OnceLock},
    time::Duration,
};

use futures::{StreamExt, stream};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, OwnedMutexGuard};
use ts_rs::TS;

use super::cli_tools::{self, CliToolId};

/// How long an `aws sts get-caller-identity` auth probe may run.
const AUTH_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Registration scope that enables SSO token refresh for sso-session profiles.
const DEFAULT_REGISTRATION_SCOPES: &str = "sso:account:access";

/// Keys VK owns inside a managed `[profile]` section. Everything else in the
/// section is preserved on rewrite.
const MANAGED_PROFILE_KEYS: &[&str] = &[
    "sso_session",
    "sso_start_url",
    "sso_region",
    "sso_account_id",
    "sso_role_name",
    "region",
    "output",
];

/// Keys VK owns inside a managed `[sso-session]` section.
const MANAGED_SESSION_KEYS: &[&str] = &["sso_start_url", "sso_region", "sso_registration_scopes"];

/// Env vars an auth probe may inherit. A whitelist so ambient `AWS_ACCESS_KEY_ID`
/// etc. in the server process can never make an unauthenticated profile look
/// authenticated (env credentials take precedence over profiles).
const PROBE_ENV_KEYS: &[&str] = &[
    "HOME",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "USER",
    "PATH",
    "TMPDIR",
    "TEMP",
    "LANG",
    "LC_ALL",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
    "no_proxy",
    "AWS_CONFIG_FILE",
    "AWS_SHARED_CREDENTIALS_FILE",
    "AWS_CA_BUNDLE",
];

#[derive(Debug, thiserror::Error)]
pub enum AwsSsoError {
    #[error("invalid {field}: {message}")]
    Validation {
        field: &'static str,
        message: String,
    },
    #[error(
        "could not parse AWS config: {0}; refusing to modify a file Vibe Kanban cannot understand"
    )]
    Parse(String),
    #[error(
        "sso-session \"{session}\" is also used by {profiles}; align their SSO start URL and region or use a different profile name prefix"
    )]
    SessionConflict { session: String, profiles: String },
    #[error("no SSO profile named \"{0}\"")]
    NotFound(String),
    #[error("no SSO session named \"{0}\"")]
    SessionNotFound(String),
    #[error("AWS SSO authentication is required for session \"{0}\"")]
    AuthenticationRequired(String),
    #[error("AWS SSO discovery failed: {0}")]
    Discovery(String),
    #[error("profile \"{0}\" already exists; confirm overwrite to replace it")]
    ProfileConflict(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// One SSO profile as VK manages it (non-secret configuration only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct AwsSsoProfile {
    pub name: String,
    pub sso_start_url: String,
    pub sso_region: String,
    pub sso_account_id: String,
    pub sso_role_name: String,
    pub region: Option<String>,
    pub output: Option<String>,
}

/// Result of the independent auth probe for one profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum AwsAuthStatus {
    /// Probe succeeded; identity is the caller's Arn.
    Authenticated { identity: String },
    /// Probe ran and reported a missing or expired SSO token.
    Unauthenticated,
    /// Probe could not run or its output was unclassifiable.
    Unknown { message: String },
    /// No aws binary resolvable (host or app-managed copy).
    CliMissing,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AwsSsoProfileStatus {
    pub profile: AwsSsoProfile,
    pub auth: AwsAuthStatus,
    /// False for `[default]` (list/sign-in only): VK never rewrites it.
    pub editable: bool,
    /// Shared AWS CLI token-cache identity used to group authentication UI.
    pub auth_scope: AwsSsoAuthScope,
}

/// Non-secret identity of the SSO token cache shared by one or more profiles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct AwsSsoAuthScope {
    /// Namespaced stable key (`session:<name>` or `start-url:<url>`).
    pub key: String,
    /// Named session, or the start URL for a legacy inline profile.
    pub label: String,
    /// Present for modern profiles backed by `[sso-session <name>]`.
    pub session_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct AwsSsoSession {
    pub name: String,
    pub sso_start_url: String,
    pub sso_region: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct AwsSsoAccount {
    pub account_id: String,
    pub account_name: String,
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct AwsProfileImportCandidate {
    pub name: String,
    pub sso_account_id: String,
    pub sso_role_name: String,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct AwsProfileImportRequest {
    pub session_name: String,
    pub region: String,
    pub output: Option<String>,
    pub profiles: Vec<AwsProfileImportCandidate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct AwsProfileImportResult {
    pub created: Vec<String>,
    pub updated: Vec<String>,
}

// ---------------------------------------------------------------------------
// Config file location and IO
// ---------------------------------------------------------------------------

pub fn aws_config_path() -> PathBuf {
    if let Some(path) = std::env::var_os("AWS_CONFIG_FILE") {
        return PathBuf::from(path);
    }
    // Match the AWS CLI's own home resolution: on Windows Python's
    // expanduser uses USERPROFILE (HOME is ignored there), on Unix HOME.
    let home = if cfg!(windows) {
        std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))
    } else {
        std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
    };
    home.map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".aws")
        .join("config")
}

fn read_config(path: &std::path::Path) -> Result<String, AwsSsoError> {
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(content),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(err) => Err(err.into()),
    }
}

fn write_config_atomic(path: &std::path::Path, content: &str) -> Result<(), AwsSsoError> {
    let dir = path
        .parent()
        .ok_or_else(|| AwsSsoError::Io(std::io::Error::other("AWS config path has no parent")))?;
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    let tmp = dir.join(format!(".vk-aws-config-{}", uuid::Uuid::new_v4()));
    {
        use std::io::Write;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&tmp)?;
        if let Err(err) = file
            .write_all(content.as_bytes())
            .and_then(|_| file.sync_all())
        {
            let _ = std::fs::remove_file(&tmp);
            return Err(err.into());
        }
    }
    if let Err(err) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(err.into());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Line-preserving document model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum SectionKind {
    /// Lines before the first section header (comments, blanks).
    Preamble,
    /// `[profile <name>]`
    Profile(String),
    /// `[default]`
    DefaultProfile,
    /// `[sso-session <name>]`
    SsoSession(String),
    /// Any other `[...]` section VK does not manage.
    Other,
}

#[derive(Debug, Clone)]
struct Section {
    kind: SectionKind,
    /// All lines including the header, with their original line endings.
    lines: Vec<String>,
}

#[derive(Debug, Clone)]
struct AwsConfigDoc {
    sections: Vec<Section>,
}

fn is_comment(trimmed: &str) -> bool {
    trimmed.starts_with('#') || trimmed.starts_with(';')
}

fn parse_header(trimmed: &str) -> Result<SectionKind, AwsSsoError> {
    let Some(inner) = trimmed
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
    else {
        return Err(AwsSsoError::Parse(format!(
            "malformed section header `{trimmed}`"
        )));
    };
    let inner = inner.trim();
    if inner == "default" {
        return Ok(SectionKind::DefaultProfile);
    }
    if let Some(name) = inner.strip_prefix("profile ") {
        return Ok(SectionKind::Profile(name.trim().to_string()));
    }
    if let Some(name) = inner.strip_prefix("sso-session ") {
        return Ok(SectionKind::SsoSession(name.trim().to_string()));
    }
    Ok(SectionKind::Other)
}

fn parse_doc(content: &str) -> Result<AwsConfigDoc, AwsSsoError> {
    let mut sections = vec![Section {
        kind: SectionKind::Preamble,
        lines: Vec::new(),
    }];
    for raw in content.split_inclusive('\n') {
        let trimmed = raw.trim();
        if trimmed.starts_with('[') {
            sections.push(Section {
                kind: parse_header(trimmed)?,
                lines: vec![raw.to_string()],
            });
            continue;
        }
        let current = sections.last_mut().expect("preamble always present");
        if !trimmed.is_empty()
            && !is_comment(trimmed)
            && !raw.starts_with([' ', '\t'])
            && !trimmed.contains('=')
        {
            return Err(AwsSsoError::Parse(format!(
                "line `{trimmed}` is neither a section header, a key = value pair, nor a comment"
            )));
        }
        current.lines.push(raw.to_string());
    }
    Ok(AwsConfigDoc { sections })
}

fn serialize_doc(doc: &AwsConfigDoc) -> String {
    doc.sections
        .iter()
        .flat_map(|section| section.lines.iter())
        .map(String::as_str)
        .collect()
}

/// Key of a top-level `key = value` body line (continuations/comments: None).
fn line_key(raw: &str) -> Option<&str> {
    if raw.starts_with([' ', '\t']) {
        return None;
    }
    let trimmed = raw.trim();
    if trimmed.is_empty() || is_comment(trimmed) {
        return None;
    }
    trimmed.split('=').next().map(str::trim)
}

/// Value for `key` in a section; the last occurrence wins (AWS CLI behavior).
fn section_value(section: &Section, key: &str) -> Option<String> {
    let mut value = None;
    for raw in section.lines.iter().skip(1) {
        if line_key(raw) == Some(key) {
            let trimmed = raw.trim();
            value = trimmed.split_once('=').map(|(_, v)| v.trim().to_string());
        }
    }
    value
}

/// Body lines to carry over when rewriting a managed section: everything that
/// is not one of `managed_keys` (comments and blank lines included), plus any
/// indented continuation lines that belong to a kept key.
fn preserved_lines(section: &Section, managed_keys: &[&str]) -> Vec<String> {
    let mut kept = Vec::new();
    let mut in_dropped_key = false;
    for raw in section.lines.iter().skip(1) {
        if raw.starts_with([' ', '\t']) && !raw.trim().is_empty() {
            if !in_dropped_key {
                kept.push(raw.clone());
            }
            continue;
        }
        match line_key(raw) {
            Some(key) if managed_keys.contains(&key) => in_dropped_key = true,
            Some(_) => {
                in_dropped_key = false;
                kept.push(raw.clone());
            }
            None => {
                in_dropped_key = false;
                // Drop pure trailing blank lines; keep comments.
                if !raw.trim().is_empty() {
                    kept.push(raw.clone());
                }
            }
        }
    }
    kept
}

fn render_profile_section(
    profile: &AwsSsoProfile,
    session: &str,
    preserved: Vec<String>,
) -> Section {
    let mut lines = vec![
        format!("[profile {}]\n", profile.name),
        format!("sso_session = {session}\n"),
        format!("sso_account_id = {}\n", profile.sso_account_id),
        format!("sso_role_name = {}\n", profile.sso_role_name),
    ];
    if let Some(region) = &profile.region {
        lines.push(format!("region = {region}\n"));
    }
    if let Some(output) = &profile.output {
        lines.push(format!("output = {output}\n"));
    }
    lines.extend(preserved);
    Section {
        kind: SectionKind::Profile(profile.name.clone()),
        lines,
    }
}

fn render_session_section(
    session: &str,
    sso_start_url: &str,
    sso_region: &str,
    registration_scopes: &str,
    preserved: Vec<String>,
) -> Section {
    let mut lines = vec![
        format!("[sso-session {session}]\n"),
        format!("sso_start_url = {sso_start_url}\n"),
        format!("sso_region = {sso_region}\n"),
        format!("sso_registration_scopes = {registration_scopes}\n"),
    ];
    lines.extend(preserved);
    Section {
        kind: SectionKind::SsoSession(session.to_string()),
        lines,
    }
}

/// Append `section` at the end of the document, keeping the existing bytes
/// untouched apart from guaranteeing a trailing newline and one blank
/// separator line.
fn append_section(doc: &mut AwsConfigDoc, section: Section) {
    let has_content = doc
        .sections
        .iter()
        .any(|s| s.lines.iter().any(|l| !l.trim().is_empty()));
    if let Some(last_line) = doc
        .sections
        .iter_mut()
        .rev()
        .find_map(|s| s.lines.last_mut())
        && !last_line.ends_with('\n')
    {
        last_line.push('\n');
    }
    let separated = doc
        .sections
        .iter()
        .flat_map(|s| s.lines.iter())
        .next_back()
        .is_none_or(|l| l.trim().is_empty());
    let mut section = section;
    if has_content && !separated {
        section.lines.insert(0, "\n".to_string());
    }
    doc.sections.push(section);
}

// ---------------------------------------------------------------------------
// Profile extraction
// ---------------------------------------------------------------------------

fn session_sections(doc: &AwsConfigDoc) -> HashMap<String, &Section> {
    doc.sections
        .iter()
        .filter_map(|s| match &s.kind {
            SectionKind::SsoSession(name) => Some((name.clone(), s)),
            _ => None,
        })
        .collect()
}

/// Profiles (named + `[default]`) that reference `session` via `sso_session`.
fn profiles_referencing_session(doc: &AwsConfigDoc, session: &str) -> Vec<String> {
    doc.sections
        .iter()
        .filter_map(|s| {
            let name = match &s.kind {
                SectionKind::Profile(name) => name.clone(),
                SectionKind::DefaultProfile => "default".to_string(),
                _ => return None,
            };
            (section_value(s, "sso_session").as_deref() == Some(session)).then_some(name)
        })
        .collect()
}

fn auth_scope_for_section(section: &Section, profile_name: &str) -> AwsSsoAuthScope {
    if let Some(session_name) = section_value(section, "sso_session") {
        return AwsSsoAuthScope {
            key: format!("session:{session_name}"),
            label: session_name.clone(),
            session_name: Some(session_name),
        };
    }
    if let Some(start_url) = section_value(section, "sso_start_url") {
        return AwsSsoAuthScope {
            key: format!("start-url:{start_url}"),
            label: start_url,
            session_name: None,
        };
    }
    // `extract_profile` rejects profiles without resolvable SSO settings. Keep
    // this fallback defensive so this shared helper remains total.
    AwsSsoAuthScope {
        key: format!("profile:{profile_name}"),
        label: profile_name.to_string(),
        session_name: None,
    }
}

fn extract_profile(
    doc: &AwsConfigDoc,
    section: &Section,
) -> Option<(AwsSsoProfile, bool, AwsSsoAuthScope)> {
    let (name, editable) = match &section.kind {
        SectionKind::Profile(name) => (name.clone(), true),
        SectionKind::DefaultProfile => ("default".to_string(), false),
        _ => return None,
    };
    let auth_scope = auth_scope_for_section(section, &name);
    let sessions = session_sections(doc);
    let (sso_start_url, sso_region) = match section_value(section, "sso_session") {
        Some(session) => {
            let session_section = sessions.get(session.as_str())?;
            (
                section_value(session_section, "sso_start_url")?,
                section_value(session_section, "sso_region")?,
            )
        }
        None => (
            section_value(section, "sso_start_url")?,
            section_value(section, "sso_region")?,
        ),
    };
    Some((
        AwsSsoProfile {
            name,
            sso_start_url,
            sso_region,
            sso_account_id: section_value(section, "sso_account_id")?,
            sso_role_name: section_value(section, "sso_role_name")?,
            region: section_value(section, "region"),
            output: section_value(section, "output"),
        },
        editable,
        auth_scope,
    ))
}

/// All fully-resolved SSO profiles with editability and shared auth scope.
fn list_profile_entries_in(
    content: &str,
) -> Result<Vec<(AwsSsoProfile, bool, AwsSsoAuthScope)>, AwsSsoError> {
    let doc = parse_doc(content)?;
    Ok(doc
        .sections
        .iter()
        .filter_map(|section| extract_profile(&doc, section))
        .collect())
}

/// All fully-resolved SSO profiles in `content`, with their editability.
fn list_profiles_in(content: &str) -> Result<Vec<(AwsSsoProfile, bool)>, AwsSsoError> {
    Ok(list_profile_entries_in(content)?
        .into_iter()
        .map(|(profile, editable, _)| (profile, editable))
        .collect())
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

fn validate_charset(
    field: &'static str,
    value: &str,
    max_len: usize,
    extra: &str,
) -> Result<(), AwsSsoError> {
    if value.is_empty() || value.len() > max_len {
        return Err(AwsSsoError::Validation {
            field,
            message: format!("must be 1-{max_len} characters"),
        });
    }
    if let Some(bad) = value
        .chars()
        .find(|c| !c.is_ascii_alphanumeric() && !extra.contains(*c))
    {
        return Err(AwsSsoError::Validation {
            field,
            message: format!("character `{bad}` is not allowed"),
        });
    }
    Ok(())
}

/// A syntactically valid profile *reference* (used for login/delete paths).
/// `allow_default` distinguishes sign-in (allowed) from writes (rejected).
pub fn validate_profile_name(name: &str, allow_default: bool) -> Result<(), AwsSsoError> {
    validate_charset("profile name", name, 128, "_.@-")?;
    if !allow_default && name == "default" {
        return Err(AwsSsoError::Validation {
            field: "profile name",
            message: "the default profile cannot be managed by Vibe Kanban".to_string(),
        });
    }
    Ok(())
}

fn validate_session(session: &AwsSsoSession) -> Result<(), AwsSsoError> {
    validate_charset("session name", &session.name, 128, "_.@-")?;
    if session.name == "default" {
        return Err(AwsSsoError::Validation {
            field: "session name",
            message: "default is reserved".to_string(),
        });
    }
    if !session.sso_start_url.starts_with("https://")
        || session.sso_start_url.len() > 2048
        || session
            .sso_start_url
            .chars()
            .any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(AwsSsoError::Validation {
            field: "sso_start_url",
            message: "must be an https:// URL without whitespace".to_string(),
        });
    }
    validate_region("sso_region", &session.sso_region)
}

fn validate_region(field: &'static str, value: &str) -> Result<(), AwsSsoError> {
    let parts: Vec<&str> = value.split('-').collect();
    let valid = parts.len() >= 3
        && parts[0].len() == 2
        && parts[0].chars().all(|c| c.is_ascii_lowercase())
        && parts[parts.len() - 1].chars().all(|c| c.is_ascii_digit())
        && !parts[parts.len() - 1].is_empty()
        && parts[1..parts.len() - 1]
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_lowercase()));
    if !valid {
        return Err(AwsSsoError::Validation {
            field,
            message: format!("`{value}` is not a valid AWS region (e.g. us-east-1)"),
        });
    }
    Ok(())
}

pub fn validate_profile(profile: &AwsSsoProfile) -> Result<(), AwsSsoError> {
    validate_profile_name(&profile.name, false)?;
    let url = &profile.sso_start_url;
    if !url.starts_with("https://")
        || url.len() > 2048
        || url.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(AwsSsoError::Validation {
            field: "sso_start_url",
            message: "must be an https:// URL without whitespace".to_string(),
        });
    }
    validate_region("sso_region", &profile.sso_region)?;
    if profile.sso_account_id.len() != 12
        || !profile.sso_account_id.chars().all(|c| c.is_ascii_digit())
    {
        return Err(AwsSsoError::Validation {
            field: "sso_account_id",
            message: "must be exactly 12 digits".to_string(),
        });
    }
    validate_charset("sso_role_name", &profile.sso_role_name, 64, "+=,.@_-")?;
    if let Some(region) = &profile.region {
        validate_region("region", region)?;
    }
    if let Some(output) = &profile.output
        && !["json", "yaml", "text", "table"].contains(&output.as_str())
    {
        return Err(AwsSsoError::Validation {
            field: "output",
            message: "must be one of json, yaml, text, table".to_string(),
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Mutations (pure, on file content)
// ---------------------------------------------------------------------------

/// Session grouping for a profile name: the prefix before the first `.`
/// (so `ai-foundry.AdministratorAccess` and `ai-foundry.ReadOnly` share one
/// session and one cached token), else the full name.
fn derived_session_name(profile_name: &str) -> &str {
    profile_name
        .split_once('.')
        .map(|(prefix, _)| prefix)
        .filter(|prefix| !prefix.is_empty())
        .unwrap_or(profile_name)
}

fn upsert_in(content: &str, profile: &AwsSsoProfile) -> Result<String, AwsSsoError> {
    let mut doc = parse_doc(content)?;
    let session = derived_session_name(&profile.name).to_string();

    let existing_session = doc
        .sections
        .iter()
        .position(|s| s.kind == SectionKind::SsoSession(session.clone()));
    match existing_session {
        Some(index) => {
            let current_url = section_value(&doc.sections[index], "sso_start_url");
            let current_region = section_value(&doc.sections[index], "sso_region");
            let unchanged = current_url.as_deref() == Some(profile.sso_start_url.as_str())
                && current_region.as_deref() == Some(profile.sso_region.as_str());
            if !unchanged {
                let others: Vec<String> = profiles_referencing_session(&doc, &session)
                    .into_iter()
                    .filter(|name| name != &profile.name)
                    .collect();
                if !others.is_empty() {
                    return Err(AwsSsoError::SessionConflict {
                        session,
                        profiles: others.join(", "),
                    });
                }
                let scopes = section_value(&doc.sections[index], "sso_registration_scopes")
                    .unwrap_or_else(|| DEFAULT_REGISTRATION_SCOPES.to_string());
                let preserved = preserved_lines(&doc.sections[index], MANAGED_SESSION_KEYS);
                doc.sections[index] = render_session_section(
                    &session,
                    &profile.sso_start_url,
                    &profile.sso_region,
                    &scopes,
                    preserved,
                );
            }
        }
        None => {
            append_section(
                &mut doc,
                render_session_section(
                    &session,
                    &profile.sso_start_url,
                    &profile.sso_region,
                    DEFAULT_REGISTRATION_SCOPES,
                    Vec::new(),
                ),
            );
        }
    }

    let existing_profile = doc
        .sections
        .iter()
        .position(|s| s.kind == SectionKind::Profile(profile.name.clone()));
    match existing_profile {
        Some(index) => {
            let preserved = preserved_lines(&doc.sections[index], MANAGED_PROFILE_KEYS);
            doc.sections[index] = render_profile_section(profile, &session, preserved);
        }
        None => {
            append_section(
                &mut doc,
                render_profile_section(profile, &session, Vec::new()),
            );
        }
    }

    Ok(serialize_doc(&doc))
}

fn list_sessions_in(content: &str) -> Result<Vec<AwsSsoSession>, AwsSsoError> {
    let doc = parse_doc(content)?;
    let mut sessions: Vec<_> = doc
        .sections
        .iter()
        .filter_map(|section| {
            let SectionKind::SsoSession(name) = &section.kind else {
                return None;
            };
            Some(AwsSsoSession {
                name: name.clone(),
                sso_start_url: section_value(section, "sso_start_url")?,
                sso_region: section_value(section, "sso_region")?,
            })
        })
        .collect();
    sessions.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(sessions)
}

fn upsert_session_in(content: &str, session: &AwsSsoSession) -> Result<String, AwsSsoError> {
    let mut doc = parse_doc(content)?;
    if let Some(index) = doc
        .sections
        .iter()
        .position(|s| s.kind == SectionKind::SsoSession(session.name.clone()))
    {
        let current_url = section_value(&doc.sections[index], "sso_start_url");
        let current_region = section_value(&doc.sections[index], "sso_region");
        if (current_url.as_deref() != Some(session.sso_start_url.as_str())
            || current_region.as_deref() != Some(session.sso_region.as_str()))
            && !profiles_referencing_session(&doc, &session.name).is_empty()
        {
            return Err(AwsSsoError::SessionConflict {
                session: session.name.clone(),
                profiles: profiles_referencing_session(&doc, &session.name).join(", "),
            });
        }
        let scopes = section_value(&doc.sections[index], "sso_registration_scopes")
            .unwrap_or_else(|| DEFAULT_REGISTRATION_SCOPES.to_string());
        let preserved = preserved_lines(&doc.sections[index], MANAGED_SESSION_KEYS);
        doc.sections[index] = render_session_section(
            &session.name,
            &session.sso_start_url,
            &session.sso_region,
            &scopes,
            preserved,
        );
    } else {
        append_section(
            &mut doc,
            render_session_section(
                &session.name,
                &session.sso_start_url,
                &session.sso_region,
                DEFAULT_REGISTRATION_SCOPES,
                Vec::new(),
            ),
        );
    }
    Ok(serialize_doc(&doc))
}

fn bulk_import_in(
    content: &str,
    request: &AwsProfileImportRequest,
) -> Result<(String, AwsProfileImportResult), AwsSsoError> {
    validate_charset("session name", &request.session_name, 128, "_.@-")?;
    validate_region("region", &request.region)?;
    if request.profiles.is_empty() {
        return Err(AwsSsoError::Validation {
            field: "profiles",
            message: "select at least one profile".to_string(),
        });
    }
    let doc = parse_doc(content)?;
    let session = doc
        .sections
        .iter()
        .find(|s| s.kind == SectionKind::SsoSession(request.session_name.clone()))
        .ok_or_else(|| AwsSsoError::SessionNotFound(request.session_name.clone()))?;
    let start_url = section_value(session, "sso_start_url")
        .ok_or_else(|| AwsSsoError::SessionNotFound(request.session_name.clone()))?;
    let sso_region = section_value(session, "sso_region")
        .ok_or_else(|| AwsSsoError::SessionNotFound(request.session_name.clone()))?;
    let existing_profiles: HashMap<String, bool> = doc
        .sections
        .iter()
        .filter_map(|s| match &s.kind {
            SectionKind::Profile(name) => Some((
                name.clone(),
                section_value(s, "sso_session").is_some()
                    || section_value(s, "sso_start_url").is_some(),
            )),
            SectionKind::DefaultProfile => Some(("default".to_string(), false)),
            _ => None,
        })
        .collect();
    let mut names = HashSet::new();
    let mut created = Vec::new();
    let mut updated = Vec::new();
    let mut profiles = Vec::new();
    for candidate in &request.profiles {
        if !names.insert(candidate.name.clone()) {
            return Err(AwsSsoError::Validation {
                field: "profiles",
                message: format!("duplicate profile name \"{}\"", candidate.name),
            });
        }
        let profile = AwsSsoProfile {
            name: candidate.name.clone(),
            sso_start_url: start_url.clone(),
            sso_region: sso_region.clone(),
            sso_account_id: candidate.sso_account_id.clone(),
            sso_role_name: candidate.sso_role_name.clone(),
            region: Some(request.region.clone()),
            output: request.output.clone(),
        };
        validate_profile(&profile)?;
        match existing_profiles.get(&candidate.name) {
            Some(true) if candidate.overwrite => updated.push(candidate.name.clone()),
            Some(_) => return Err(AwsSsoError::ProfileConflict(candidate.name.clone())),
            None => created.push(candidate.name.clone()),
        }
        profiles.push(profile);
    }
    let mut updated_content = content.to_string();
    for profile in &profiles {
        // Use an explicit session independent of the profile's edited name.
        let mut next = parse_doc(&updated_content)?;
        let existing = next
            .sections
            .iter()
            .position(|s| s.kind == SectionKind::Profile(profile.name.clone()));
        match existing {
            Some(index) => {
                let preserved = preserved_lines(&next.sections[index], MANAGED_PROFILE_KEYS);
                next.sections[index] =
                    render_profile_section(profile, &request.session_name, preserved);
            }
            None => append_section(
                &mut next,
                render_profile_section(profile, &request.session_name, Vec::new()),
            ),
        }
        updated_content = serialize_doc(&next);
    }
    created.sort();
    updated.sort();
    Ok((updated_content, AwsProfileImportResult { created, updated }))
}

fn delete_in(content: &str, name: &str) -> Result<String, AwsSsoError> {
    let mut doc = parse_doc(content)?;
    let index = doc
        .sections
        .iter()
        .position(|s| s.kind == SectionKind::Profile(name.to_string()))
        .ok_or_else(|| AwsSsoError::NotFound(name.to_string()))?;
    let section = &doc.sections[index];
    let is_sso = section_value(section, "sso_session").is_some()
        || section_value(section, "sso_start_url").is_some();
    if !is_sso {
        return Err(AwsSsoError::NotFound(name.to_string()));
    }
    let session = section_value(section, "sso_session");
    doc.sections.remove(index);

    if let Some(session) = session
        && profiles_referencing_session(&doc, &session).is_empty()
        && let Some(session_index) = doc
            .sections
            .iter()
            .position(|s| s.kind == SectionKind::SsoSession(session.clone()))
    {
        doc.sections.remove(session_index);
    }
    Ok(serialize_doc(&doc))
}

// ---------------------------------------------------------------------------
// Auth probe
// ---------------------------------------------------------------------------

fn classify_probe_success(stdout: &[u8]) -> AwsAuthStatus {
    let parsed: Result<serde_json::Value, _> = serde_json::from_slice(stdout);
    match parsed.ok().and_then(|value| {
        value
            .get("Arn")
            .and_then(|arn| arn.as_str())
            .map(str::to_string)
    }) {
        Some(identity) => AwsAuthStatus::Authenticated { identity },
        None => AwsAuthStatus::Unknown {
            message: "auth check returned unrecognized output".to_string(),
        },
    }
}

fn classify_probe_failure(stderr: &str) -> AwsAuthStatus {
    let lowered = stderr.to_lowercase();
    const UNAUTHENTICATED_MARKERS: &[&str] = &[
        "token has expired",
        "sso session",
        "error loading sso token",
        "failed to refresh",
        "unable to locate credentials",
        "expiredtoken",
        "is expired",
        "token file does not exist",
    ];
    if UNAUTHENTICATED_MARKERS
        .iter()
        .any(|marker| lowered.contains(marker))
    {
        return AwsAuthStatus::Unauthenticated;
    }
    let summary: String = stderr.trim().chars().take(200).collect();
    AwsAuthStatus::Unknown {
        message: if summary.is_empty() {
            "auth check failed".to_string()
        } else {
            summary
        },
    }
}

async fn probe_profile_auth(profile_name: &str) -> AwsAuthStatus {
    let Some(executable) = cli_tools::effective_binary_for(CliToolId::Aws).await else {
        return AwsAuthStatus::CliMissing;
    };
    let mut command = tokio::process::Command::new(executable);
    command
        .args([
            "sts",
            "get-caller-identity",
            "--profile",
            profile_name,
            "--output",
            "json",
        ])
        .env_clear();
    for key in PROBE_ENV_KEYS {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    match tokio::time::timeout(AUTH_PROBE_TIMEOUT, command.kill_on_drop(true).output()).await {
        Ok(Ok(output)) if output.status.success() => classify_probe_success(&output.stdout),
        Ok(Ok(output)) => classify_probe_failure(&String::from_utf8_lossy(&output.stderr)),
        Ok(Err(err)) => AwsAuthStatus::Unknown {
            message: format!("could not run auth check: {err}"),
        },
        Err(_) => AwsAuthStatus::Unknown {
            message: "auth check timed out".to_string(),
        },
    }
}

// ---------------------------------------------------------------------------
// Login lock and command
// ---------------------------------------------------------------------------

fn login_lock_for_key(key: &str) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<StdMutex<HashMap<String, Arc<Mutex<()>>>>> = OnceLock::new();
    let map = LOCKS.get_or_init(Default::default);
    let mut guard = map.lock().expect("aws login lock map poisoned");
    guard
        .entry(key.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

/// One active `aws sso login` per lock key per server process. The key is the
/// profile's SSO session (see [`AwsLoginCommand::lock_key`]): profiles that
/// share a session share one token cache, so concurrent logins for them would
/// race the AWS CLI's cache writes.
pub fn try_begin_profile_login(key: &str) -> Option<OwnedMutexGuard<()>> {
    login_lock_for_key(key).try_lock_owned().ok()
}

#[derive(Debug, Clone)]
pub struct AwsLoginCommand {
    pub executable: PathBuf,
    pub args: Vec<String>,
    /// Login mutual-exclusion key: the profile's `sso_session` name when it
    /// has one, else the profile name (legacy inline profiles have their own
    /// token cache entry keyed by start URL).
    pub lock_key: String,
    /// Environment the login PTY needs beyond the PTY's minimal allowlist —
    /// currently the AWS file-location overrides, so `aws sso login` acts on
    /// the same config file VK manages.
    pub env: HashMap<String, String>,
}

/// Lock key for a profile in `content`: its `sso_session` value when present,
/// else its inline `sso_start_url` (the legacy token cache is keyed by start
/// URL, so legacy profiles sharing a URL share a cache entry), else the name.
fn login_lock_key_in(content: &str, name: &str) -> Result<String, AwsSsoError> {
    let doc = parse_doc(content)?;
    let section = doc.sections.iter().find(|s| match &s.kind {
        SectionKind::Profile(profile_name) => profile_name == name,
        SectionKind::DefaultProfile => name == "default",
        _ => false,
    });
    Ok(section
        .map(|section| auth_scope_for_section(section, name))
        .map(|scope| scope.session_name.unwrap_or_else(|| scope.label.clone()))
        .unwrap_or_else(|| name.to_string()))
}

/// Build the sign-in command for a configured profile. Fails when the name is
/// malformed, the profile is not in the config file, or no aws binary
/// resolves. The executable and args are built entirely server-side.
pub async fn login_command_for_profile(name: &str) -> Result<AwsLoginCommand, AwsSsoError> {
    validate_profile_name(name, true)?;
    let content = read_config(&aws_config_path())?;
    let known = list_profiles_in(&content)?
        .into_iter()
        .any(|(profile, _)| profile.name == name);
    if !known {
        return Err(AwsSsoError::NotFound(name.to_string()));
    }
    let executable = cli_tools::effective_binary_for(CliToolId::Aws)
        .await
        .ok_or_else(|| AwsSsoError::Validation {
            field: "aws",
            message: "the AWS CLI is not available on this machine; install it from CLI Tools"
                .to_string(),
        })?;
    let mut env = HashMap::new();
    // File-location overrides so login acts on the same config VK manages,
    // plus the Windows home variables the PTY's minimal allowlist omits
    // (the AWS CLI needs one of them to find %USERPROFILE%\.aws).
    for key in [
        "AWS_CONFIG_FILE",
        "AWS_SHARED_CREDENTIALS_FILE",
        "USERPROFILE",
        "HOMEDRIVE",
        "HOMEPATH",
    ] {
        if let Some(value) = std::env::var(key).ok().filter(|v| !v.is_empty()) {
            env.insert(key.to_string(), value);
        }
    }
    Ok(AwsLoginCommand {
        executable,
        // --use-device-code: the default Authorization Code flow expects a
        // browser on the same machine as the CLI (it opens one and waits on
        // a local loopback callback), which fails on a headless/remote VK
        // server with "If you are unable to open the URL on this device,
        // run this command again with the '--use-device-code' option."
        // Device Code always prints a URL + code the user completes on any
        // device, so it works identically whether VK is local or remote —
        // the same reasoning behind this codebase's `az login
        // --use-device-code`.
        args: vec![
            "sso".to_string(),
            "login".to_string(),
            "--profile".to_string(),
            name.to_string(),
            "--use-device-code".to_string(),
        ],
        lock_key: login_lock_key_in(&content, name)?,
        env,
    })
}

fn login_environment() -> HashMap<String, String> {
    let mut env = HashMap::new();
    for key in [
        "AWS_CONFIG_FILE",
        "AWS_SHARED_CREDENTIALS_FILE",
        "USERPROFILE",
        "HOMEDRIVE",
        "HOMEPATH",
    ] {
        if let Some(value) = std::env::var(key).ok().filter(|v| !v.is_empty()) {
            env.insert(key.to_string(), value);
        }
    }
    env
}

pub async fn login_command_for_session(name: &str) -> Result<AwsLoginCommand, AwsSsoError> {
    validate_charset("session name", name, 128, "_.@-")?;
    let content = read_config(&aws_config_path())?;
    if !list_sessions_in(&content)?.iter().any(|s| s.name == name) {
        return Err(AwsSsoError::SessionNotFound(name.to_string()));
    }
    let executable = cli_tools::effective_binary_for(CliToolId::Aws)
        .await
        .ok_or_else(|| AwsSsoError::Validation {
            field: "aws",
            message: "the AWS CLI is not available on this machine; install it from CLI Tools"
                .to_string(),
        })?;
    Ok(AwsLoginCommand {
        executable,
        args: vec![
            "sso".to_string(),
            "login".to_string(),
            "--sso-session".to_string(),
            name.to_string(),
            "--use-device-code".to_string(),
        ],
        lock_key: name.to_string(),
        env: login_environment(),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CachedSsoToken {
    start_url: String,
    region: String,
    access_token: String,
    expires_at: String,
}

struct SecretToken(String);

impl std::fmt::Debug for SecretToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretToken([REDACTED])")
    }
}

fn aws_sso_cache_path() -> PathBuf {
    let home = if cfg!(windows) {
        std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))
    } else {
        std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
    };
    home.map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".aws")
        .join("sso")
        .join("cache")
}

fn cached_token_for(session: &AwsSsoSession) -> Result<SecretToken, AwsSsoError> {
    let entries = match std::fs::read_dir(aws_sso_cache_path()) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(AwsSsoError::AuthenticationRequired(session.name.clone()));
        }
        Err(err) => return Err(err.into()),
    };
    let now = chrono::Utc::now();
    let mut matches = Vec::new();
    for entry in entries.flatten().take(1024) {
        if entry.path().extension().and_then(|v| v.to_str()) != Some("json") {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.len() > 1024 * 1024 {
            continue;
        }
        let Ok(bytes) = std::fs::read(entry.path()) else {
            continue;
        };
        let Ok(cached) = serde_json::from_slice::<CachedSsoToken>(&bytes) else {
            continue;
        };
        let Ok(expires) = chrono::DateTime::parse_from_rfc3339(&cached.expires_at) else {
            continue;
        };
        if cached.start_url == session.sso_start_url
            && cached.region == session.sso_region
            && expires.with_timezone(&chrono::Utc) > now
            && !cached.access_token.is_empty()
        {
            matches.push((expires, cached.access_token));
        }
    }
    matches.sort_by_key(|(expires, _)| *expires);
    matches
        .pop()
        .map(|(_, token)| SecretToken(token))
        .ok_or_else(|| AwsSsoError::AuthenticationRequired(session.name.clone()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountsPage {
    #[serde(default)]
    account_list: Vec<AccountInfo>,
    next_token: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountInfo {
    account_id: String,
    account_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RolesPage {
    #[serde(default)]
    role_list: Vec<RoleInfo>,
    next_token: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoleInfo {
    role_name: String,
}

async fn get_portal_page<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
    token: &SecretToken,
    query: &[(&str, String)],
) -> Result<T, AwsSsoError> {
    let response = client
        .get(url)
        .header("x-amz-sso_bearer_token", &token.0)
        .query(query)
        .send()
        .await
        .map_err(|_| AwsSsoError::Discovery("AWS could not be reached".to_string()))?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(AwsSsoError::AuthenticationRequired(
            "selected session".to_string(),
        ));
    }
    if !response.status().is_success() {
        return Err(AwsSsoError::Discovery(format!(
            "AWS returned status {}",
            response.status().as_u16()
        )));
    }
    response
        .json::<T>()
        .await
        .map_err(|_| AwsSsoError::Discovery("AWS returned an invalid response".to_string()))
}

async fn discover_with(
    base_url: &str,
    session: &AwsSsoSession,
    token: SecretToken,
) -> Result<Vec<AwsSsoAccount>, AwsSsoError> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| AwsSsoError::Discovery("could not initialize AWS client".to_string()))?;
    let mut accounts = Vec::new();
    let mut next = None;
    loop {
        let mut query = vec![("max_result", "100".to_string())];
        if let Some(value) = next.take() {
            query.push(("next_token", value));
        }
        let page: AccountsPage = get_portal_page(
            &client,
            &format!("{base_url}/assignment/accounts"),
            &token,
            &query,
        )
        .await?;
        accounts.extend(page.account_list);
        next = page.next_token;
        if next.is_none() {
            break;
        }
    }
    let results = stream::iter(accounts.into_iter().map(|account| {
        let client = client.clone();
        let token = SecretToken(token.0.clone());
        let base_url = base_url.to_string();
        async move {
            let mut roles = Vec::new();
            let mut next = None;
            loop {
                let mut query = vec![
                    ("account_id", account.account_id.clone()),
                    ("max_result", "100".to_string()),
                ];
                if let Some(value) = next.take() {
                    query.push(("next_token", value));
                }
                let page: RolesPage = get_portal_page(
                    &client,
                    &format!("{base_url}/assignment/roles"),
                    &token,
                    &query,
                )
                .await?;
                roles.extend(page.role_list.into_iter().map(|r| r.role_name));
                next = page.next_token;
                if next.is_none() {
                    break;
                }
            }
            roles.sort_by_key(|r| r.to_lowercase());
            Ok::<_, AwsSsoError>(AwsSsoAccount {
                account_id: account.account_id,
                account_name: account.account_name,
                roles,
            })
        }
    }))
    .buffer_unordered(8)
    .collect::<Vec<_>>()
    .await;
    let mut catalog: Vec<_> = results.into_iter().collect::<Result<_, _>>()?;
    catalog.sort_by(|a, b| {
        a.account_name
            .to_lowercase()
            .cmp(&b.account_name.to_lowercase())
            .then_with(|| a.account_id.cmp(&b.account_id))
    });
    let _ = session;
    Ok(catalog)
}

// ---------------------------------------------------------------------------
// Public file-backed API
// ---------------------------------------------------------------------------

pub async fn list_profile_statuses() -> Result<Vec<AwsSsoProfileStatus>, AwsSsoError> {
    let content = read_config(&aws_config_path())?;
    let profiles = list_profile_entries_in(&content)?;
    let statuses = futures::future::join_all(
        profiles
            .iter()
            .map(|(profile, _, _)| async { probe_profile_auth(&profile.name).await }),
    )
    .await;
    Ok(profiles
        .into_iter()
        .zip(statuses)
        .map(
            |((profile, editable, auth_scope), auth)| AwsSsoProfileStatus {
                profile,
                auth,
                editable,
                auth_scope,
            },
        )
        .collect())
}

pub fn list_sessions() -> Result<Vec<AwsSsoSession>, AwsSsoError> {
    list_sessions_in(&read_config(&aws_config_path())?)
}

pub async fn prepare_session(session: &AwsSsoSession) -> Result<AwsSsoSession, AwsSsoError> {
    validate_session(session)?;
    let _guard = config_write_lock().lock().await;
    let path = aws_config_path();
    let content = read_config(&path)?;
    let updated = upsert_session_in(&content, session)?;
    write_config_atomic(&path, &updated)?;
    Ok(session.clone())
}

pub async fn discover_catalog(name: &str) -> Result<Vec<AwsSsoAccount>, AwsSsoError> {
    let session = list_sessions()?
        .into_iter()
        .find(|s| s.name == name)
        .ok_or_else(|| AwsSsoError::SessionNotFound(name.to_string()))?;
    let token = cached_token_for(&session)?;
    let base_url = format!("https://portal.sso.{}.amazonaws.com", session.sso_region);
    discover_with(&base_url, &session, token).await
}

pub async fn import_profiles(
    request: &AwsProfileImportRequest,
) -> Result<AwsProfileImportResult, AwsSsoError> {
    let _guard = config_write_lock().lock().await;
    let path = aws_config_path();
    let content = read_config(&path)?;
    let (updated, result) = bulk_import_in(&content, request)?;
    write_config_atomic(&path, &updated)?;
    Ok(result)
}

/// Probe one profile's auth state (used after a login attempt).
pub async fn profile_status(name: &str) -> Result<AwsSsoProfileStatus, AwsSsoError> {
    let content = read_config(&aws_config_path())?;
    let (profile, editable, auth_scope) = list_profile_entries_in(&content)?
        .into_iter()
        .find(|(profile, _, _)| profile.name == name)
        .ok_or_else(|| AwsSsoError::NotFound(name.to_string()))?;
    let auth = probe_profile_auth(name).await;
    Ok(AwsSsoProfileStatus {
        profile,
        auth,
        editable,
        auth_scope,
    })
}

/// Serializes read-modify-write cycles on the config file within this server
/// process, so two concurrent saves cannot both read the same original bytes
/// and drop each other's profile on rename.
fn config_write_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub async fn upsert_profile(profile: &AwsSsoProfile) -> Result<AwsSsoProfile, AwsSsoError> {
    validate_profile(profile)?;
    let _guard = config_write_lock().lock().await;
    let path = aws_config_path();
    let content = read_config(&path)?;
    let updated = upsert_in(&content, profile)?;
    write_config_atomic(&path, &updated)?;
    Ok(profile.clone())
}

pub async fn delete_profile(name: &str) -> Result<(), AwsSsoError> {
    validate_profile_name(name, false)?;
    let _guard = config_write_lock().lock().await;
    let path = aws_config_path();
    let content = read_config(&path)?;
    let updated = delete_in(&content, name)?;
    write_config_atomic(&path, &updated)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(name: &str) -> AwsSsoProfile {
        AwsSsoProfile {
            name: name.to_string(),
            sso_start_url: "https://ai-foundry.awsapps.com/start".to_string(),
            sso_region: "us-east-1".to_string(),
            sso_account_id: "123456789012".to_string(),
            sso_role_name: "AdministratorAccess".to_string(),
            region: Some("us-east-1".to_string()),
            output: Some("json".to_string()),
        }
    }

    // -- parsing / round-trip ------------------------------------------------

    #[test]
    fn untouched_content_round_trips_byte_for_byte() {
        let content = "# hand-written config\n\n[default]\nregion = eu-west-1\n\n[profile keys]\naws_access_key_id = AKIA123\naws_secret_access_key = shhh\n\n[custom section]\nanything = goes\n";
        let doc = parse_doc(content).unwrap();
        assert_eq!(serialize_doc(&doc), content);
    }

    #[test]
    fn unparseable_file_is_rejected() {
        assert!(matches!(
            parse_doc("[unclosed section\nkey = value\n"),
            Err(AwsSsoError::Parse(_))
        ));
        assert!(matches!(
            parse_doc("[profile ok]\nthis is not a key value line\n"),
            Err(AwsSsoError::Parse(_))
        ));
    }

    #[test]
    fn crlf_and_missing_trailing_newline_round_trip() {
        let content = "[profile a.b]\r\nsso_start_url = https://x/start\r\nsso_region = us-east-1\r\nsso_account_id = 123456789012\r\nsso_role_name = R";
        let doc = parse_doc(content).unwrap();
        assert_eq!(serialize_doc(&doc), content);
    }

    // -- listing -------------------------------------------------------------

    #[test]
    fn lists_modern_legacy_and_default_profiles_only() {
        let content = "\
[sso-session org]
sso_start_url = https://org.awsapps.com/start
sso_region = us-east-1
sso_registration_scopes = sso:account:access

[profile org.Admin]
sso_session = org
sso_account_id = 123456789012
sso_role_name = AdministratorAccess
region = us-west-2

[profile legacy]
sso_start_url = https://legacy.awsapps.com/start
sso_region = eu-west-1
sso_account_id = 210987654321
sso_role_name = ReadOnly

[profile keys]
aws_access_key_id = AKIA123

[default]
sso_session = org
sso_account_id = 123456789012
sso_role_name = ReadOnly
";
        let profiles = list_profiles_in(content).unwrap();
        let names: Vec<(&str, bool)> = profiles
            .iter()
            .map(|(p, editable)| (p.name.as_str(), *editable))
            .collect();
        assert_eq!(
            names,
            vec![("org.Admin", true), ("legacy", true), ("default", false)]
        );
        let admin = &profiles[0].0;
        assert_eq!(admin.sso_start_url, "https://org.awsapps.com/start");
        assert_eq!(admin.sso_region, "us-east-1");
        assert_eq!(admin.region.as_deref(), Some("us-west-2"));
        let legacy = &profiles[1].0;
        assert_eq!(legacy.sso_start_url, "https://legacy.awsapps.com/start");
        assert_eq!(legacy.output, None);
    }

    #[test]
    fn profile_with_missing_session_is_not_listed() {
        let content = "[profile broken]\nsso_session = ghost\nsso_account_id = 123456789012\nsso_role_name = R\n";
        assert!(list_profiles_in(content).unwrap().is_empty());
    }

    // -- upsert --------------------------------------------------------------

    #[test]
    fn upsert_into_empty_file_writes_modern_form() {
        let updated = upsert_in("", &profile("ai-foundry.AdministratorAccess")).unwrap();
        assert_eq!(
            updated,
            "\
[sso-session ai-foundry]
sso_start_url = https://ai-foundry.awsapps.com/start
sso_region = us-east-1
sso_registration_scopes = sso:account:access

[profile ai-foundry.AdministratorAccess]
sso_session = ai-foundry
sso_account_id = 123456789012
sso_role_name = AdministratorAccess
region = us-east-1
output = json
"
        );
        // The written file must itself list back.
        let listed = list_profiles_in(&updated).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].0, profile("ai-foundry.AdministratorAccess"));
    }

    #[test]
    fn upsert_preserves_unrelated_content_byte_for_byte() {
        let content = "# my config\n[default]\nregion = eu-west-1\n\n[custom thing]\nfoo = bar\n";
        let updated = upsert_in(content, &profile("ai-foundry.Admin")).unwrap();
        assert!(updated.starts_with(content));
        assert!(updated.contains("[sso-session ai-foundry]"));
        assert!(updated.contains("[profile ai-foundry.Admin]"));
    }

    #[test]
    fn upsert_without_trailing_newline_still_separates_sections() {
        let content = "[custom]\nfoo = bar";
        let updated = upsert_in(content, &profile("org.Admin")).unwrap();
        assert!(updated.starts_with("[custom]\nfoo = bar\n\n[sso-session org]\n"));
    }

    #[test]
    fn profiles_sharing_a_prefix_share_one_session() {
        let first = upsert_in("", &profile("ai-foundry.AdministratorAccess")).unwrap();
        let second = upsert_in(&first, &profile("ai-foundry.ReadOnly")).unwrap();
        assert_eq!(second.matches("[sso-session ai-foundry]").count(), 1);
        assert_eq!(list_profiles_in(&second).unwrap().len(), 2);
    }

    #[test]
    fn conflicting_session_edit_is_rejected_naming_other_profiles() {
        let content = upsert_in("", &profile("ai-foundry.AdministratorAccess")).unwrap();
        let content = upsert_in(&content, &profile("ai-foundry.ReadOnly")).unwrap();
        let mut moved = profile("ai-foundry.ReadOnly");
        moved.sso_start_url = "https://elsewhere.awsapps.com/start".to_string();
        let err = upsert_in(&content, &moved).unwrap_err();
        match err {
            AwsSsoError::SessionConflict { session, profiles } => {
                assert_eq!(session, "ai-foundry");
                assert_eq!(profiles, "ai-foundry.AdministratorAccess");
            }
            other => panic!("expected SessionConflict, got {other:?}"),
        }
    }

    #[test]
    fn sole_referent_may_move_its_session() {
        let content = upsert_in("", &profile("ai-foundry.Admin")).unwrap();
        let mut moved = profile("ai-foundry.Admin");
        moved.sso_start_url = "https://elsewhere.awsapps.com/start".to_string();
        let updated = upsert_in(&content, &moved).unwrap();
        assert!(updated.contains("sso_start_url = https://elsewhere.awsapps.com/start"));
        assert_eq!(updated.matches("[sso-session ai-foundry]").count(), 1);
    }

    #[test]
    fn editing_a_legacy_profile_converts_it_and_keeps_unknown_keys() {
        let content = "\
[profile legacy]
# keep me
sso_start_url = https://legacy.awsapps.com/start
sso_region = eu-west-1
sso_account_id = 210987654321
sso_role_name = ReadOnly
cli_pager =
";
        let mut edited = profile("legacy");
        edited.sso_start_url = "https://legacy.awsapps.com/start".to_string();
        edited.sso_region = "eu-west-1".to_string();
        let updated = upsert_in(content, &edited).unwrap();
        assert!(updated.contains("[sso-session legacy]"));
        assert!(updated.contains("sso_session = legacy"));
        assert!(!updated.contains("[profile legacy]\nsso_start_url"));
        assert!(updated.contains("# keep me\n"));
        assert!(updated.contains("cli_pager =\n"));
    }

    #[test]
    fn upsert_preserves_custom_registration_scopes() {
        let content = "\
[sso-session org]
sso_start_url = https://old.awsapps.com/start
sso_region = us-east-1
sso_registration_scopes = sso:account:access codewhisperer:analysis
";
        let mut p = profile("org.Admin");
        p.sso_start_url = "https://new.awsapps.com/start".to_string();
        let updated = upsert_in(content, &p).unwrap();
        assert!(
            updated.contains("sso_registration_scopes = sso:account:access codewhisperer:analysis")
        );
        assert!(updated.contains("sso_start_url = https://new.awsapps.com/start"));
    }

    // -- delete --------------------------------------------------------------

    #[test]
    fn delete_removes_profile_and_orphaned_session() {
        let content = upsert_in("", &profile("org.Admin")).unwrap();
        let updated = delete_in(&content, "org.Admin").unwrap();
        assert!(!updated.contains("[profile org.Admin]"));
        assert!(!updated.contains("[sso-session org]"));
    }

    #[test]
    fn delete_keeps_session_still_referenced_by_others() {
        let content = upsert_in("", &profile("org.Admin")).unwrap();
        let content = upsert_in(&content, &profile("org.ReadOnly")).unwrap();
        let updated = delete_in(&content, "org.Admin").unwrap();
        assert!(!updated.contains("[profile org.Admin]"));
        assert!(updated.contains("[profile org.ReadOnly]"));
        assert!(updated.contains("[sso-session org]"));
    }

    #[test]
    fn delete_leaves_unrelated_content_untouched() {
        let prefix = "# untouched\n[custom]\nfoo = bar\n";
        let content = upsert_in(prefix, &profile("org.Admin")).unwrap();
        let updated = delete_in(&content, "org.Admin").unwrap();
        assert!(updated.starts_with(prefix));
    }

    #[test]
    fn delete_missing_or_non_sso_profile_is_not_found() {
        assert!(matches!(
            delete_in("", "ghost"),
            Err(AwsSsoError::NotFound(_))
        ));
        let content = "[profile keys]\naws_access_key_id = AKIA123\n";
        assert!(matches!(
            delete_in(content, "keys"),
            Err(AwsSsoError::NotFound(_))
        ));
    }

    // -- validation ----------------------------------------------------------

    #[test]
    fn validation_matrix() {
        assert!(validate_profile(&profile("ai-foundry.AdministratorAccess")).is_ok());

        let mut p = profile("bad name");
        assert!(validate_profile(&p).is_err());
        p = profile("inject]\n[default");
        assert!(validate_profile(&p).is_err());
        p = profile("default");
        assert!(validate_profile(&p).is_err());

        p = profile("ok");
        p.sso_start_url = "http://insecure/start".to_string();
        assert!(validate_profile(&p).is_err());
        p = profile("ok");
        p.sso_start_url = "https://x/start\ninject = true".to_string();
        assert!(validate_profile(&p).is_err());

        p = profile("ok");
        p.sso_region = "US-EAST-1".to_string();
        assert!(validate_profile(&p).is_err());
        p = profile("ok");
        p.sso_region = "us-gov-west-1".to_string();
        assert!(validate_profile(&p).is_ok());

        p = profile("ok");
        p.sso_account_id = "12345".to_string();
        assert!(validate_profile(&p).is_err());
        p = profile("ok");
        p.sso_account_id = "12345678901a".to_string();
        assert!(validate_profile(&p).is_err());

        p = profile("ok");
        p.sso_role_name = String::new();
        assert!(validate_profile(&p).is_err());

        p = profile("ok");
        p.output = Some("xml".to_string());
        assert!(validate_profile(&p).is_err());
        p = profile("ok");
        p.output = None;
        p.region = None;
        assert!(validate_profile(&p).is_ok());
    }

    #[test]
    fn default_is_a_valid_login_reference_but_not_writable() {
        assert!(validate_profile_name("default", true).is_ok());
        assert!(validate_profile_name("default", false).is_err());
    }

    // -- probe classification ------------------------------------------------

    #[test]
    fn probe_classification_from_canned_outputs() {
        assert_eq!(
            classify_probe_success(
                br#"{"UserId":"X","Account":"123456789012","Arn":"arn:aws:sts::123456789012:assumed-role/AdministratorAccess/user"}"#
            ),
            AwsAuthStatus::Authenticated {
                identity: "arn:aws:sts::123456789012:assumed-role/AdministratorAccess/user"
                    .to_string()
            }
        );
        assert!(matches!(
            classify_probe_success(b"not json"),
            AwsAuthStatus::Unknown { .. }
        ));
        assert_eq!(
            classify_probe_failure("Error loading SSO Token: Token for ai-foundry does not exist"),
            AwsAuthStatus::Unauthenticated
        );
        assert_eq!(
            classify_probe_failure(
                "The SSO session associated with this profile has expired or is otherwise invalid."
            ),
            AwsAuthStatus::Unauthenticated
        );
        assert_eq!(
            classify_probe_failure(
                "Unable to locate credentials. You can configure credentials by running \"aws configure\"."
            ),
            AwsAuthStatus::Unauthenticated
        );
        assert_eq!(
            classify_probe_failure(
                "An error occurred (ExpiredToken) when calling the GetCallerIdentity operation"
            ),
            AwsAuthStatus::Unauthenticated
        );
        assert!(matches!(
            classify_probe_failure("Could not connect to the endpoint URL"),
            AwsAuthStatus::Unknown { .. }
        ));
    }

    // -- session derivation / login lock -------------------------------------

    #[test]
    fn session_name_is_prefix_before_first_dot() {
        assert_eq!(
            derived_session_name("ai-foundry.AdministratorAccess"),
            "ai-foundry"
        );
        assert_eq!(derived_session_name("plain"), "plain");
        assert_eq!(derived_session_name("a.b.c"), "a");
    }

    #[test]
    fn login_lock_key_is_the_shared_session() {
        let content = upsert_in("", &profile("org.Admin")).unwrap();
        let content = upsert_in(&content, &profile("org.ReadOnly")).unwrap();
        // Profiles sharing one sso-session (one token cache) share one key.
        assert_eq!(login_lock_key_in(&content, "org.Admin").unwrap(), "org");
        assert_eq!(login_lock_key_in(&content, "org.ReadOnly").unwrap(), "org");
        // Legacy inline profiles lock on their start URL: the legacy token
        // cache is keyed by URL, so same-URL profiles share one cache entry.
        let legacy = "[profile legacy]\nsso_start_url = https://x/start\nsso_region = us-east-1\nsso_account_id = 123456789012\nsso_role_name = R\n[profile legacy2]\nsso_start_url = https://x/start\nsso_region = us-east-1\nsso_account_id = 210987654321\nsso_role_name = R\n";
        assert_eq!(
            login_lock_key_in(legacy, "legacy").unwrap(),
            "https://x/start"
        );
        assert_eq!(
            login_lock_key_in(legacy, "legacy").unwrap(),
            login_lock_key_in(legacy, "legacy2").unwrap()
        );
        // An SSO-configured [default] locks on its session too.
        let with_default = format!(
            "{content}\n[default]\nsso_session = org\nsso_account_id = 123456789012\nsso_role_name = R\n"
        );
        assert_eq!(login_lock_key_in(&with_default, "default").unwrap(), "org");
    }

    #[test]
    fn auth_scope_distinguishes_named_sessions_and_groups_legacy_urls() {
        let named = "\
[sso-session first]\n\
sso_start_url = https://x/start\n\
sso_region = us-east-1\n\
[sso-session second]\n\
sso_start_url = https://x/start\n\
sso_region = us-east-1\n\
[profile a]\n\
sso_session = first\n\
sso_account_id = 123456789012\n\
sso_role_name = R\n\
[profile b]\n\
sso_session = second\n\
sso_account_id = 210987654321\n\
sso_role_name = R\n";
        let entries = list_profile_entries_in(named).unwrap();
        assert_eq!(entries[0].2.key, "session:first");
        assert_eq!(entries[0].2.session_name.as_deref(), Some("first"));
        assert_eq!(entries[1].2.key, "session:second");

        let legacy = "\
[profile legacy]\n\
sso_start_url = https://x/start\n\
sso_region = us-east-1\n\
sso_account_id = 123456789012\n\
sso_role_name = R\n\
[profile legacy2]\n\
sso_start_url = https://x/start\n\
sso_region = us-east-1\n\
sso_account_id = 210987654321\n\
sso_role_name = R\n";
        let entries = list_profile_entries_in(legacy).unwrap();
        assert_eq!(entries[0].2.key, "start-url:https://x/start");
        assert_eq!(entries[0].2, entries[1].2);
        assert_eq!(entries[0].2.session_name, None);
    }

    #[test]
    fn login_lock_is_per_profile() {
        let first = try_begin_profile_login("lock-test.Admin").expect("first login owns lock");
        assert!(try_begin_profile_login("lock-test.Admin").is_none());
        assert!(try_begin_profile_login("lock-test.Other").is_some());
        drop(first);
        assert!(try_begin_profile_login("lock-test.Admin").is_some());
    }

    #[test]
    fn session_prepare_preserves_unrelated_content() {
        let session = AwsSsoSession {
            name: "work".into(),
            sso_start_url: "https://work.awsapps.com/start".into(),
            sso_region: "us-east-1".into(),
        };
        let original = "# keep me\n[custom]\nfoo = bar\n";
        let updated = upsert_session_in(original, &session).unwrap();
        assert!(updated.starts_with(original));
        assert!(updated.contains("[sso-session work]"));
        assert_eq!(list_sessions_in(&updated).unwrap(), vec![session]);
    }

    #[test]
    fn bulk_import_is_explicit_session_and_all_or_nothing() {
        let session = AwsSsoSession {
            name: "company".into(),
            sso_start_url: "https://company.awsapps.com/start".into(),
            sso_region: "us-east-1".into(),
        };
        let content = upsert_session_in("# original\n", &session).unwrap();
        let request = AwsProfileImportRequest {
            session_name: session.name,
            region: "us-west-2".into(),
            output: Some("json".into()),
            profiles: vec![
                AwsProfileImportCandidate {
                    name: "dev.Admin".into(),
                    sso_account_id: "123456789012".into(),
                    sso_role_name: "Admin".into(),
                    overwrite: false,
                },
                AwsProfileImportCandidate {
                    name: "prod.ReadOnly".into(),
                    sso_account_id: "210987654321".into(),
                    sso_role_name: "ReadOnly".into(),
                    overwrite: false,
                },
            ],
        };
        let (updated, result) = bulk_import_in(&content, &request).unwrap();
        assert_eq!(result.created, vec!["dev.Admin", "prod.ReadOnly"]);
        assert_eq!(updated.matches("sso_session = company").count(), 2);
        assert!(updated.starts_with("# original\n"));

        let mut duplicate = request;
        duplicate.profiles[1].name = "dev.Admin".into();
        assert!(bulk_import_in(&content, &duplicate).is_err());
        assert_eq!(content.matches("[profile ").count(), 0);
    }

    #[tokio::test]
    async fn discovery_paginates_accounts_and_roles_and_sorts_complete_result() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for _ in 0..4 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = vec![0; 8192];
                let read = socket.read(&mut bytes).await.unwrap();
                let request = String::from_utf8_lossy(&bytes[..read]);
                assert!(request.contains("x-amz-sso_bearer_token: fake-token"));
                let body = if request.starts_with("GET /assignment/accounts?") {
                    if request.contains("next_token=more") {
                        r#"{"accountList":[{"accountId":"111111111111","accountName":"Alpha"}]}"#
                    } else {
                        r#"{"accountList":[{"accountId":"222222222222","accountName":"Zulu"}],"nextToken":"more"}"#
                    }
                } else if request.contains("account_id=111111111111") {
                    r#"{"roleList":[{"roleName":"ReadOnly"}]}"#
                } else {
                    r#"{"roleList":[{"roleName":"Admin"}]}"#
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            }
        });
        let session = AwsSsoSession {
            name: "test".into(),
            sso_start_url: "https://test.awsapps.com/start".into(),
            sso_region: "us-east-1".into(),
        };
        let catalog = discover_with(
            &format!("http://{address}"),
            &session,
            SecretToken("fake-token".into()),
        )
        .await
        .unwrap();
        server.await.unwrap();
        assert_eq!(catalog[0].account_name, "Alpha");
        assert_eq!(catalog[0].roles, vec!["ReadOnly"]);
        assert_eq!(catalog[1].account_name, "Zulu");
        assert_eq!(catalog[1].roles, vec!["Admin"]);
    }

    // -- file IO -------------------------------------------------------------

    #[cfg(unix)]
    #[test]
    fn atomic_write_creates_restrictive_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("aws").join("config");
        write_config_atomic(&path, "[default]\nregion = us-east-1\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "[default]\nregion = us-east-1\n"
        );
        let file_mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(file_mode & 0o777, 0o600);
        let dir_mode = std::fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(dir_mode & 0o777, 0o700);
        // No temp files left behind.
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
    }

    #[test]
    fn missing_config_file_reads_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_config(&dir.path().join("nope")).unwrap(), "");
    }
}
