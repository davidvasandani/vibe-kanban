//! Unattended re-authentication: repair expired credentials with no human.
//!
//! Adapts the operator's macOS `aws-sso-reauth` skill to the cluster. Where
//! that skill drove a throwaway Chrome with a service-account token, this
//! reuses the machinery [`entra_mint`] already ships: the firecrawl browser
//! with the persistent Entra profile (so most runs are silent) and 1Password
//! Connect for the password and TOTP.
//!
//! Targets are server-defined (see [`ReauthTargetId`]); a caller only names
//! one. A run is owned by a detached task, not by the request that started it,
//! so a Codex-sized tool deadline never cancels a sign-in halfway. Success is
//! only ever claimed after an independent probe of the vendor's own store.
//!
//! Safety rails (homelab constitution principle 129): definitive refusals are
//! never retried, and once a target is refused only an operator-triggered run
//! may try it again; the sweep backs off exponentially; an unrepairable
//! target emits one fixed escalation line that the deployment pages on.

pub mod acli;
pub mod aws;
pub mod sgsc;
pub mod sweep;

use std::{
    collections::HashMap,
    fmt,
    str::FromStr,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{
    aws_sso,
    cli_tools::{self, CliToolAuthState, CliToolError, CliToolId, CliToolLoginPlan},
    entra_mint::{self, EntraError, Progress},
};

/// Fixed text the deployment's alert unit matches. Changing it silently
/// disables paging, so it is pinned by a test.
pub const ESCALATION_MARKER: &str = "vk-reauth: operator action required";

/// Transcript lines kept per run.
const TRANSCRIPT_CAP: usize = 200;

// ---------------------------------------------------------------------------
// Identity
// ---------------------------------------------------------------------------

/// A re-authentication target. The wire form is `<kind>:<name>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ReauthTargetId {
    /// A `[sso-session <name>]`: one sign-in refreshes every profile in it.
    AwsSession(String),
    /// A legacy inline SSO profile (its token is keyed by start URL).
    AwsProfile(String),
    CliTool(CliToolId),
    /// One sgsc-mcp gateway backend (`dp`, `sf`, …).
    Sgsc(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ReauthError {
    #[error("invalid re-auth target: {0}")]
    InvalidTarget(String),
    #[error("not configured: {0}")]
    NotConfigured(String),
    #[error("{0}")]
    Busy(String),
    #[error(transparent)]
    Entra(#[from] EntraError),
    #[error("{0}")]
    Failed(String),
}

impl ReauthError {
    /// A refusal that must not be retried without an operator.
    fn is_refusal(&self) -> bool {
        matches!(self, ReauthError::Entra(EntraError::Refused(_)))
    }
}

impl From<CliToolError> for ReauthError {
    fn from(err: CliToolError) -> Self {
        match err {
            CliToolError::Entra(EntraError::Config(message)) => ReauthError::NotConfigured(message),
            CliToolError::Entra(e) => ReauthError::Entra(e),
            CliToolError::Unsupported(_, reason) => ReauthError::NotConfigured(reason),
            other => ReauthError::Failed(other.to_string()),
        }
    }
}

impl From<aws_sso::AwsSsoError> for ReauthError {
    fn from(err: aws_sso::AwsSsoError) -> Self {
        ReauthError::Failed(err.to_string())
    }
}

/// sgsc backend names: short, lowercase, and safe inside a URL path.
fn validate_backend(name: &str) -> Result<(), ReauthError> {
    let ok = (1..=32).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_');
    if ok {
        Ok(())
    } else {
        Err(ReauthError::InvalidTarget(format!(
            "sgsc backend '{name}' must match [a-z0-9_-]{{1,32}}"
        )))
    }
}

impl FromStr for ReauthTargetId {
    type Err = ReauthError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (kind, name) = s
            .split_once(':')
            .ok_or_else(|| ReauthError::InvalidTarget(format!("'{s}' has no kind prefix")))?;
        let invalid = |e: aws_sso::AwsSsoError| ReauthError::InvalidTarget(e.to_string());
        match kind {
            "aws-sso" => {
                aws_sso::validate_session_name(name).map_err(invalid)?;
                Ok(Self::AwsSession(name.to_string()))
            }
            "aws-profile" => {
                aws_sso::validate_profile_name(name, true).map_err(invalid)?;
                Ok(Self::AwsProfile(name.to_string()))
            }
            "cli-tool" => {
                let id: CliToolId = serde_json::from_value(serde_json::Value::String(
                    name.to_string(),
                ))
                .map_err(|_| ReauthError::InvalidTarget(format!("unknown CLI tool '{name}'")))?;
                if !cli_tools::unattended_login(id) {
                    return Err(ReauthError::InvalidTarget(format!(
                        "CLI tool '{name}' cannot be signed in unattended"
                    )));
                }
                Ok(Self::CliTool(id))
            }
            "sgsc" => {
                validate_backend(name)?;
                Ok(Self::Sgsc(name.to_string()))
            }
            _ => Err(ReauthError::InvalidTarget(format!(
                "unknown target kind '{kind}'"
            ))),
        }
    }
}

impl fmt::Display for ReauthTargetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AwsSession(name) => write!(f, "aws-sso:{name}"),
            Self::AwsProfile(name) => write!(f, "aws-profile:{name}"),
            Self::CliTool(id) => {
                let wire = serde_json::to_value(id)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_default();
                write!(f, "cli-tool:{wire}")
            }
            Self::Sgsc(name) => write!(f, "sgsc:{name}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReauthKind {
    AwsSso,
    CliTool,
    Sgsc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReauthAuthState {
    Authenticated,
    Unauthenticated,
    /// No verdict: the check could not run, or the target has no check.
    Unknown,
    /// A prerequisite is missing; `auth_message` names it.
    NotConfigured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReauthRunOutcome {
    Running,
    Succeeded,
    Failed,
    /// The flow finished, but the independent probe still says no.
    VerificationFailed,
    /// A definitive refusal (wrong password, Conditional Access, locked).
    Refused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReauthTrigger {
    /// The operator, from Settings.
    Manual,
    /// An agent, through the MCP tool.
    Agent,
    /// The background sweep.
    Sweep,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ReauthRun {
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub outcome: ReauthRunOutcome,
    pub trigger: ReauthTrigger,
    /// Named error or verification note, verbatim. Never holds a secret.
    pub message: Option<String>,
    /// Redacted step log.
    pub transcript: Vec<String>,
    /// True in a response when the call joined a run already in progress.
    pub already_running: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ReauthTargetStatus {
    pub id: String,
    pub kind: ReauthKind,
    pub label: String,
    /// Whether the automatic sweep repairs this target.
    pub swept: bool,
    pub auth_state: ReauthAuthState,
    pub auth_message: Option<String>,
    /// Refused earlier: agents and the sweep will not retry until an
    /// operator re-runs it from Settings.
    pub refused: bool,
    pub last_run: Option<ReauthRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ReauthOverview {
    /// Sweep period, or `None` when the sweep is off.
    #[ts(type = "number | null")]
    pub sweep_interval_secs: Option<u64>,
    pub targets: Vec<ReauthTargetStatus>,
}

/// One target's run, as returned by `POST /api/reauth/run`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ReauthRunReport {
    pub id: String,
    pub run: ReauthRun,
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

#[derive(Default)]
struct TargetState {
    last_run: Option<ReauthRun>,
    running: bool,
    refused: bool,
    consecutive_failures: u32,
    next_eligible: Option<Instant>,
    escalated: bool,
}

fn registry() -> &'static Mutex<HashMap<ReauthTargetId, TargetState>> {
    static REGISTRY: OnceLock<Mutex<HashMap<ReauthTargetId, TargetState>>> = OnceLock::new();
    REGISTRY.get_or_init(Default::default)
}

fn with_registry<R>(f: impl FnOnce(&mut HashMap<ReauthTargetId, TargetState>) -> R) -> R {
    let mut guard = registry().lock().unwrap_or_else(|e| e.into_inner());
    f(&mut guard)
}

/// What `claim` decided.
enum Claim {
    /// The caller now owns a fresh run.
    Started(ReauthRun),
    /// A run was already in progress; no second flow was started.
    Joined(ReauthRun),
    /// Refused earlier and this trigger may not retry it.
    Blocked(ReauthRun),
}

fn claim(
    states: &mut HashMap<ReauthTargetId, TargetState>,
    id: &ReauthTargetId,
    trigger: ReauthTrigger,
    now: DateTime<Utc>,
) -> Claim {
    let state = states.entry(id.clone()).or_default();
    if state.running
        && let Some(run) = &state.last_run
    {
        let mut joined = run.clone();
        joined.already_running = true;
        return Claim::Joined(joined);
    }
    if state.refused {
        if trigger != ReauthTrigger::Manual {
            let reason = state
                .last_run
                .as_ref()
                .and_then(|r| r.message.clone())
                .unwrap_or_else(|| "refused".to_string());
            return Claim::Blocked(ReauthRun {
                started_at: now,
                finished_at: Some(now),
                outcome: ReauthRunOutcome::Refused,
                trigger,
                message: Some(format!(
                    "not retried: an earlier attempt was refused ({reason}). \
                     Fix the credential, then re-run it from Settings."
                )),
                transcript: Vec::new(),
                already_running: false,
            });
        }
        // An operator re-running from Settings is the explicit reset.
        state.refused = false;
    }
    let run = ReauthRun {
        started_at: now,
        finished_at: None,
        outcome: ReauthRunOutcome::Running,
        trigger,
        message: None,
        transcript: Vec::new(),
        already_running: false,
    };
    state.running = true;
    state.last_run = Some(run.clone());
    Claim::Started(run)
}

/// What finishing a run should do beyond recording it.
#[derive(Debug, PartialEq, Eq)]
enum Escalation {
    None,
    Emit,
}

fn finish(
    states: &mut HashMap<ReauthTargetId, TargetState>,
    id: &ReauthTargetId,
    outcome: ReauthRunOutcome,
    message: Option<String>,
    now: (DateTime<Utc>, Instant),
    sweep_interval: Duration,
) -> Escalation {
    let state = states.entry(id.clone()).or_default();
    state.running = false;
    let trigger = state
        .last_run
        .as_ref()
        .map(|r| r.trigger)
        .unwrap_or(ReauthTrigger::Manual);
    if let Some(run) = state.last_run.as_mut() {
        run.outcome = outcome;
        run.finished_at = Some(now.0);
        run.message = message;
    }
    if outcome == ReauthRunOutcome::Succeeded {
        state.consecutive_failures = 0;
        state.next_eligible = None;
        state.escalated = false;
        state.refused = false;
        return Escalation::None;
    }
    if outcome == ReauthRunOutcome::Refused {
        state.refused = true;
    }
    if trigger == ReauthTrigger::Sweep {
        state.consecutive_failures += 1;
        state.next_eligible = Some(sweep::next_eligible(
            now.1,
            state.consecutive_failures,
            sweep_interval,
        ));
    }
    if sweep::should_escalate(
        outcome == ReauthRunOutcome::Refused,
        state.consecutive_failures,
        state.escalated,
    ) {
        state.escalated = true;
        Escalation::Emit
    } else {
        Escalation::None
    }
}

fn append_transcript(id: &ReauthTargetId, line: String) {
    with_registry(|states| {
        if let Some(run) = states.get_mut(id).and_then(|s| s.last_run.as_mut())
            && run.transcript.len() < TRANSCRIPT_CAP
        {
            run.transcript.push(line);
        }
    });
}

/// Strip query strings from anything URL-shaped, as a last line of defence:
/// engines already log only hosts and paths.
fn redact_line(line: &str) -> String {
    line.trim_end_matches(['\r', '\n'])
        .split(' ')
        .map(
            |word| match word.find("https://").or_else(|| word.find("http://")) {
                Some(at) => format!("{}{}", &word[..at], entra_mint::redact_url(&word[at..])),
                None => word.to_string(),
            },
        )
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

struct Target {
    id: ReauthTargetId,
    kind: ReauthKind,
    label: String,
    swept: bool,
    auth_state: ReauthAuthState,
    auth_message: Option<String>,
}

/// Whether the browser-driven engines have what they need.
fn entra_missing() -> Option<String> {
    entra_mint::EntraConfig::from_env()
        .err()
        .map(|e| e.to_string())
}

fn sgsc_backends() -> Vec<String> {
    std::env::var("VK_SGSC_BACKENDS")
        .unwrap_or_default()
        .split(',')
        .map(|b| b.trim().to_string())
        .filter(|b| validate_backend(b).is_ok())
        .collect()
}

/// Whether a failed probe's output shows an authentication failure (as
/// opposed to a network outage, rate limit or missing permission). The CLI
/// probes report only an exit code to the card; this is the evidence that
/// makes a failure safe to act on unattended.
fn auth_failure_confirmed(id: CliToolId, output: &str) -> bool {
    let text = output.to_lowercase();
    let any = |needles: &[&str]| needles.iter().any(|n| text.contains(n));
    match id {
        CliToolId::Az => any(&[
            "az login",
            "aadsts",
            "interactionrequired",
            "interaction_required",
            "refresh token",
            "token has expired",
            "tokencreatedwithoutdatedpolicies",
        ]),
        // The probe prints nothing and exits 1 when there is no Graph
        // context at all. The profile block warns when its token refresh
        // fails: a rejected refresh token (invalid_grant / AADSTS) is an
        // expiry, anything else (an unreachable endpoint) is not.
        CliToolId::GraphPowershell10 => text.trim().is_empty() || any(&["invalid_grant", "aadsts"]),
        CliToolId::MgcBeta => any(&[
            "unauthorized",
            "401",
            "authentication",
            "login required",
            "no account",
            "interactive",
        ]),
        CliToolId::Acli => any(&["unauthorized", "auth login", "401", "not authenticated"]),
        _ => false,
    }
}

async fn discover_cli_tools() -> Vec<Target> {
    let ids: Vec<CliToolId> = CliToolId::ALL
        .into_iter()
        .filter(|id| cli_tools::unattended_login(*id))
        .collect();
    // Profiles written before refresh failures were made visible would make
    // an outage look like "never signed in"; bring the vk block up to date
    // before trusting an empty Graph probe.
    cli_tools::migrate_graph_powershell_profile();
    let statuses = futures::future::join_all(
        ids.iter()
            .map(|id| cli_tools::status_with_probe_output(*id)),
    )
    .await;
    ids.into_iter()
        .zip(statuses)
        .map(|(id, (status, probe_output))| {
            let config_gap = if id == CliToolId::Acli {
                acli::ApiTokenConfig::from_env()
                    .err()
                    .map(|e| e.to_string())
            } else {
                entra_missing()
            };
            let (auth_state, auth_message) = match (config_gap, status.auth_state) {
                (Some(gap), _) => (ReauthAuthState::NotConfigured, Some(gap)),
                (None, CliToolAuthState::Authenticated) => (ReauthAuthState::Authenticated, None),
                (None, CliToolAuthState::Unauthenticated) => {
                    if auth_failure_confirmed(id, probe_output.as_deref().unwrap_or_default()) {
                        (ReauthAuthState::Unauthenticated, None)
                    } else {
                        // A failed probe without an authentication error is
                        // an outage or a permission problem, not an expiry:
                        // signing in again would not fix it, so it is never
                        // repaired automatically.
                        (
                            ReauthAuthState::Unknown,
                            Some(
                                "the check failed without an authentication error; \
                                 not repaired automatically"
                                    .to_string(),
                            ),
                        )
                    }
                }
                (None, CliToolAuthState::Unknown) => {
                    (ReauthAuthState::Unknown, status.auth_message.clone())
                }
                (None, CliToolAuthState::Unsupported) => {
                    (ReauthAuthState::NotConfigured, status.auth_message.clone())
                }
            };
            let scope_note = if id == CliToolId::Acli {
                " (coordinator only)"
            } else {
                ""
            };
            Target {
                id: ReauthTargetId::CliTool(id),
                kind: ReauthKind::CliTool,
                label: format!("{}{scope_note}", status.display_name),
                swept: true,
                auth_state,
                auth_message,
            }
        })
        .collect()
}

/// Aggregate per-profile states into one scope state: any unauthenticated
/// profile means the shared token needs repair.
fn aggregate_aws(states: &[&aws_sso::AwsAuthStatus]) -> (ReauthAuthState, Option<String>) {
    use aws_sso::AwsAuthStatus as S;
    if states.iter().any(|s| matches!(s, S::CliMissing)) {
        return (
            ReauthAuthState::NotConfigured,
            Some("the AWS CLI is not available on this machine".to_string()),
        );
    }
    if states.iter().any(|s| matches!(s, S::Unauthenticated)) {
        return (ReauthAuthState::Unauthenticated, None);
    }
    if !states.is_empty() && states.iter().all(|s| matches!(s, S::Authenticated { .. })) {
        return (ReauthAuthState::Authenticated, None);
    }
    let message = states.iter().find_map(|s| match s {
        S::Unknown { message } => Some(message.clone()),
        _ => None,
    });
    (ReauthAuthState::Unknown, message)
}

/// The one target that stands for `profile`'s shared token: its session, or
/// for a legacy profile the first profile (in config order) sharing its
/// start-URL scope. Discovery and starts both resolve through this, so one
/// credential never has two registry entries (or two refusal gates).
fn canonical_aws_target(
    scopes: &[(String, aws_sso::AwsSsoAuthScope)],
    profile: &str,
) -> Option<ReauthTargetId> {
    let (_, scope) = scopes.iter().find(|(name, _)| name == profile)?;
    if let Some(session) = &scope.session_name {
        return Some(ReauthTargetId::AwsSession(session.clone()));
    }
    let (first, _) = scopes.iter().find(|(_, other)| other.key == scope.key)?;
    Some(ReauthTargetId::AwsProfile(first.clone()))
}

/// Scope targets with their member profiles, in config order.
fn aws_groups(scopes: &[(String, aws_sso::AwsSsoAuthScope)]) -> Vec<(ReauthTargetId, Vec<String>)> {
    let mut groups: Vec<(ReauthTargetId, Vec<String>)> = Vec::new();
    for (profile, _) in scopes {
        let Some(id) = canonical_aws_target(scopes, profile) else {
            continue;
        };
        match groups.iter_mut().find(|(i, _)| *i == id) {
            Some((_, members)) => members.push(profile.clone()),
            None => groups.push((id, vec![profile.clone()])),
        }
    }
    groups
}

fn aws_target(
    id: ReauthTargetId,
    members: usize,
    auth_state: ReauthAuthState,
    auth_message: Option<String>,
) -> Target {
    let name = match &id {
        ReauthTargetId::AwsSession(n) | ReauthTargetId::AwsProfile(n) => n.clone(),
        _ => String::new(),
    };
    Target {
        label: format!(
            "AWS SSO · {name} ({members} profile{})",
            if members == 1 { "" } else { "s" }
        ),
        id,
        kind: ReauthKind::AwsSso,
        swept: true,
        auth_state,
        auth_message,
    }
}

/// Every member profile is probed, through the same bounded, lazily
/// admitted probe stream Settings uses: one successful STS call is not proof
/// for the scope, because the CLI can answer from cached role credentials
/// after the shared SSO token has expired.
async fn discover_aws() -> Vec<Target> {
    let Ok(statuses) = aws_sso::list_profile_statuses().await else {
        return Vec::new();
    };
    let scopes: Vec<(String, aws_sso::AwsSsoAuthScope)> = statuses
        .iter()
        .map(|s| (s.profile.name.clone(), s.auth_scope.clone()))
        .collect();
    let missing = entra_missing();
    aws_groups(&scopes)
        .into_iter()
        .map(|(id, members)| {
            let auths: Vec<&aws_sso::AwsAuthStatus> = statuses
                .iter()
                .filter(|s| members.contains(&s.profile.name))
                .map(|s| &s.auth)
                .collect();
            let (state, message) = match &missing {
                Some(gap) => (ReauthAuthState::NotConfigured, Some(gap.clone())),
                None => aggregate_aws(&auths),
            };
            aws_target(id, members.len(), state, message)
        })
        .collect()
}

fn discover_sgsc() -> Vec<Target> {
    let gap = entra_missing().or_else(|| sgsc::onboard_template().err().map(|e| e.to_string()));
    sgsc_backends()
        .into_iter()
        .map(|backend| Target {
            label: format!("sgsc-mcp · {backend}"),
            id: ReauthTargetId::Sgsc(backend),
            kind: ReauthKind::Sgsc,
            // No non-mutating probe exists, so a sweep could not tell a
            // healthy grant from an expired one and would spend factors on
            // every tick (principle 129). On demand only.
            swept: false,
            auth_state: if gap.is_some() {
                ReauthAuthState::NotConfigured
            } else {
                ReauthAuthState::Unknown
            },
            auth_message: gap.clone().or_else(|| {
                Some("the gateway has no status check; re-authenticate on demand".to_string())
            }),
        })
        .collect()
}

/// How long AWS probing may take during discovery. Under a slow network the
/// shared four-probe budget can take minutes over dozens of profiles; past
/// this, the AWS targets are still listed, just unprobed.
const AWS_DISCOVERY_BUDGET: Duration = Duration::from_secs(40);

/// AWS scope targets straight from the config file, without probing.
fn aws_targets_unprobed(message: &str) -> Vec<Target> {
    let Ok(scopes) = aws_sso::profile_scopes() else {
        return Vec::new();
    };
    aws_groups(&scopes)
        .into_iter()
        .map(|(id, members)| {
            aws_target(
                id,
                members.len(),
                ReauthAuthState::Unknown,
                Some(message.to_string()),
            )
        })
        .collect()
}

/// All targets. `listing` bounds AWS probing so an interactive list (an
/// agent tool, the Settings card) always answers; repair paths wait for real
/// probe results, so an expired scope is never hidden behind a timeout.
async fn discover(listing: bool) -> Vec<Target> {
    let aws = async {
        if !listing {
            return discover_aws().await;
        }
        tokio::time::timeout(AWS_DISCOVERY_BUDGET, discover_aws())
            .await
            .unwrap_or_else(|_| {
                aws_targets_unprobed("the AWS status check did not finish in time; state unknown")
            })
    };
    let (tools, aws) = tokio::join!(discover_cli_tools(), aws);
    let mut all = aws;
    all.extend(tools);
    all.extend(discover_sgsc());
    all
}

fn status_of(target: Target, states: &HashMap<ReauthTargetId, TargetState>) -> ReauthTargetStatus {
    let state = states.get(&target.id);
    ReauthTargetStatus {
        id: target.id.to_string(),
        kind: target.kind,
        label: target.label,
        swept: target.swept,
        auth_state: target.auth_state,
        auth_message: target.auth_message,
        refused: state.is_some_and(|s| s.refused),
        last_run: state.and_then(|s| s.last_run.clone()),
    }
}

/// Every target with its current (independently probed) state.
pub async fn list_targets() -> ReauthOverview {
    let targets = discover(true).await;
    let targets =
        with_registry(|states| targets.into_iter().map(|t| status_of(t, states)).collect());
    ReauthOverview {
        sweep_interval_secs: sweep::sweep_interval_from_env().map(|d| d.as_secs()),
        targets,
    }
}

/// The last run of every target that has run, with no probing at all: cheap
/// enough for a UI to poll while a run is in progress.
pub fn recent_runs() -> Vec<ReauthRunReport> {
    with_registry(|states| {
        let mut runs: Vec<ReauthRunReport> = states
            .iter()
            .filter_map(|(id, state)| {
                state.last_run.clone().map(|run| ReauthRunReport {
                    id: id.to_string(),
                    run,
                })
            })
            .collect();
        runs.sort_by(|a, b| a.id.cmp(&b.id));
        runs
    })
}

/// Targets the sweep (or "re-authenticate everything") would repair now.
async fn expired_swept_targets(trigger: ReauthTrigger) -> Vec<ReauthTargetId> {
    let now = Instant::now();
    let targets = discover(false).await;
    with_registry(|states| {
        targets
            .into_iter()
            .filter(|t| t.swept && t.auth_state == ReauthAuthState::Unauthenticated)
            .filter(|t| selectable(states.get(&t.id), trigger, now))
            .map(|t| t.id)
            .collect()
    })
}

/// Whether an expired target may join a bulk run for `trigger`. Refused
/// targets already escalated, so agents and the sweep leave them alone; an
/// operator's manual "re-authenticate all" is exactly the reset that retries
/// them. Only the sweep honours its own backoff.
fn selectable(state: Option<&TargetState>, trigger: ReauthTrigger, now: Instant) -> bool {
    let Some(state) = state else {
        return true;
    };
    if state.refused && trigger != ReauthTrigger::Manual {
        return false;
    }
    trigger != ReauthTrigger::Sweep || state.next_eligible.is_none_or(|at| at <= now)
}

// ---------------------------------------------------------------------------
// Running
// ---------------------------------------------------------------------------

/// The listed target `id` stands for. Only listed targets may start: a run
/// nobody can see could neither be polled nor, once refused, reset from
/// Settings. An AWS profile resolves to the scope target discovery lists.
fn canonicalize(id: ReauthTargetId) -> Result<ReauthTargetId, ReauthError> {
    let unlisted = |id: &ReauthTargetId| {
        ReauthError::InvalidTarget(format!(
            "{id} is not a re-auth target on this host (see list_reauth_targets)"
        ))
    };
    match &id {
        ReauthTargetId::CliTool(_) => Ok(id),
        ReauthTargetId::Sgsc(backend) => {
            if sgsc_backends().iter().any(|b| b == backend) {
                Ok(id)
            } else {
                Err(unlisted(&id))
            }
        }
        ReauthTargetId::AwsSession(session) => {
            if aws_sso::profile_scopes()?
                .iter()
                .any(|(_, scope)| scope.session_name.as_deref() == Some(session))
            {
                Ok(id)
            } else {
                Err(unlisted(&id))
            }
        }
        ReauthTargetId::AwsProfile(profile) => {
            canonical_aws_target(&aws_sso::profile_scopes()?, profile).ok_or_else(|| unlisted(&id))
        }
    }
}

/// Start (or join) a run of a listed target. Returns immediately; the run is
/// owned by a detached task and outlives the request.
pub fn start_listed(
    id: ReauthTargetId,
    trigger: ReauthTrigger,
) -> Result<ReauthRunReport, ReauthError> {
    Ok(start(canonicalize(id)?, trigger))
}

/// Start (or join) a run of `id`. Returns immediately; the run is owned by a
/// detached task and outlives the request.
fn start(id: ReauthTargetId, trigger: ReauthTrigger) -> ReauthRunReport {
    let claim = with_registry(|states| claim(states, &id, trigger, Utc::now()));
    let run = match claim {
        Claim::Joined(run) | Claim::Blocked(run) => run,
        Claim::Started(run) => {
            let task_id = id.clone();
            tokio::spawn(async move { execute(task_id).await });
            run
        }
    };
    ReauthRunReport {
        id: id.to_string(),
        run,
    }
}

/// Start every swept target that is currently unauthenticated.
///
/// All targets are claimed up front (so each reports `running` at once), but
/// one task runs them strictly one after another. Every browser engine needs
/// the single Entra profile, so starting them together would only park later
/// targets on the profile lock — an AWS device code expiring while it waits —
/// and turn one fleet-wide expiry into a burst of failures.
pub async fn start_expired(trigger: ReauthTrigger) -> Vec<ReauthRunReport> {
    let ids = expired_swept_targets(trigger).await;
    let now = Utc::now();
    let mut reports = Vec::with_capacity(ids.len());
    let mut owned = Vec::new();
    with_registry(|states| {
        for id in ids {
            let run = match claim(states, &id, trigger, now) {
                Claim::Joined(run) | Claim::Blocked(run) => run,
                Claim::Started(run) => {
                    owned.push(id.clone());
                    run
                }
            };
            reports.push(ReauthRunReport {
                id: id.to_string(),
                run,
            });
        }
    });
    if !owned.is_empty() {
        tokio::spawn(async move {
            for id in owned {
                execute(id).await;
            }
        });
    }
    reports
}

/// Wait up to `max` for the named runs to settle, then report them.
pub async fn wait(reports: Vec<ReauthRunReport>, max: Duration) -> Vec<ReauthRunReport> {
    let ids: Vec<ReauthTargetId> = reports.iter().filter_map(|r| r.id.parse().ok()).collect();
    let deadline = tokio::time::Instant::now() + max;
    loop {
        let settled = with_registry(|states| {
            ids.iter()
                .all(|id| states.get(id).is_none_or(|s| !s.running))
        });
        if settled || tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    with_registry(|states| {
        reports
            .into_iter()
            .map(|mut report| {
                if let Ok(id) = report.id.parse::<ReauthTargetId>()
                    && let Some(run) = states.get(&id).and_then(|s| s.last_run.clone())
                    && run.started_at >= report.run.started_at
                {
                    let joined = report.run.already_running;
                    report.run = run;
                    report.run.already_running = joined;
                }
                report
            })
            .collect()
    })
}

async fn execute(id: ReauthTargetId) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let sink_id = id.clone();
    let sink = tokio::spawn(async move {
        while let Some(line) = rx.recv().await {
            append_transcript(&sink_id, redact_line(&line));
        }
    });
    let progress = Progress::new(tx);
    let (trigger, requested_at) = with_registry(|states| {
        states
            .get(&id)
            .and_then(|s| s.last_run.as_ref())
            .map(|r| (r.trigger, r.started_at))
    })
    .unwrap_or((ReauthTrigger::Manual, Utc::now()));
    let shares_entra = uses_entra(&id);
    // Check, attempt and latch as one step across every Entra-backed run:
    // otherwise two concurrent targets both pass the check, and the second
    // resubmits a password the first just saw refused.
    let _entra_turn = if shares_entra {
        Some(entra_run_lock().lock().await)
    } else {
        None
    };
    let latched = shares_entra
        .then(|| entra_gate(trigger, entra_refusal().as_ref(), requested_at))
        .flatten();
    let gated = latched.is_some();
    let result = match latched {
        Some(reason) => {
            progress.say(&reason);
            Err(ReauthError::Entra(EntraError::Refused(reason)))
        }
        None => match tokio::time::timeout(ENGINE_TIMEOUT, run_engine(&id, &progress)).await {
            Ok(result) => result.map_err(normalize_refusal),
            // Dropping the engine kills its children (kill_on_drop) and
            // releases the browser profile, so a hung step cannot hold the
            // Entra run lock — and every later repair — forever.
            Err(_) => Err(ReauthError::Failed(format!(
                "the sign-in did not finish within {} minutes",
                ENGINE_TIMEOUT.as_secs() / 60
            ))),
        },
    };
    if shares_entra {
        match &result {
            // Only a real attempt latches; a gated one just repeats it.
            Err(ReauthError::Entra(EntraError::Refused(reason))) if !gated => {
                set_entra_refusal(Some(reason.clone()));
            }
            Ok(_) => set_entra_refusal(None),
            _ => {}
        }
    }
    let (outcome, message) = match result {
        Ok(note) => match verify(&id).await {
            Ok(()) => (ReauthRunOutcome::Succeeded, note),
            Err(why) => (ReauthRunOutcome::VerificationFailed, Some(why)),
        },
        Err(err) if err.is_refusal() => (ReauthRunOutcome::Refused, Some(err.to_string())),
        Err(err) => (ReauthRunOutcome::Failed, Some(err.to_string())),
    };
    drop(progress);
    let _ = sink.await;
    let interval = sweep::sweep_interval_from_env().unwrap_or(sweep::MIN_INTERVAL);
    let escalation = with_registry(|states| {
        finish(
            states,
            &id,
            outcome,
            message.clone(),
            (Utc::now(), Instant::now()),
            interval,
        )
    });
    tracing::info!(target = %id, ?outcome, "unattended re-authentication finished");
    if escalation == Escalation::Emit {
        tracing::warn!(
            "{ESCALATION_MARKER} target={id} outcome={} reason={}",
            serde_json::to_value(outcome)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default(),
            message.as_deref().unwrap_or("unknown")
        );
    }
}

/// Longest one run may take end to end (AWS alone may use three attempts of
/// a few minutes each).
const ENGINE_TIMEOUT: Duration = Duration::from_secs(20 * 60);

/// A definitive refusal is a refusal whichever path reported it — a page, an
/// OAuth redirect, the token endpoint, or a tool's own output — so the shared
/// gate and escalation see it every time.
fn normalize_refusal(err: ReauthError) -> ReauthError {
    if err.is_refusal() {
        return err;
    }
    match entra_mint::refusal_in(&err.to_string()) {
        Some(reason) => ReauthError::Entra(EntraError::Refused(format!("{reason}: {err}"))),
        None => err,
    }
}

/// Every browser-driven target signs in as the same Entra account with the
/// same 1Password factors; acli uses its own API token.
fn uses_entra(id: &ReauthTargetId) -> bool {
    !matches!(id, ReauthTargetId::CliTool(CliToolId::Acli))
}

/// One Entra-backed run at a time, held from the refusal check through the
/// latch update. They already share one browser profile, so this costs no
/// concurrency that existed.
fn entra_run_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(Default::default)
}

/// The last definitive Entra refusal (with when it was recorded), shared by
/// every Entra-backed target.
#[derive(Clone)]
struct EntraRefusal {
    reason: String,
    at: DateTime<Utc>,
}

fn entra_refusal_cell() -> &'static Mutex<Option<EntraRefusal>> {
    static CELL: OnceLock<Mutex<Option<EntraRefusal>>> = OnceLock::new();
    CELL.get_or_init(Default::default)
}

fn entra_refusal() -> Option<EntraRefusal> {
    entra_refusal_cell()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

fn set_entra_refusal(reason: Option<String>) {
    *entra_refusal_cell()
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = reason.map(|reason| EntraRefusal {
        reason,
        at: Utc::now(),
    });
}

/// Whether a run must settle without signing in because the shared Entra
/// account was refused. A refusal is about the account, not the target: one
/// stale password must not be resubmitted once per queued target (that is
/// the lockout). An operator run from Settings may retry a refusal recorded
/// *before* it was requested — that is the reset — but a refusal hit after
/// it was queued (by an earlier target in the same "re-authenticate all")
/// stops it too.
fn entra_gate(
    trigger: ReauthTrigger,
    refusal: Option<&EntraRefusal>,
    requested_at: DateTime<Utc>,
) -> Option<String> {
    let refusal = refusal?;
    let operator_reset = trigger == ReauthTrigger::Manual && refusal.at < requested_at;
    (!operator_reset).then(|| {
        format!(
            "not attempted: the shared Entra sign-in was refused ({}). \
             Fix the credential, then re-run a target from Settings.",
            refusal.reason
        )
    })
}

/// Run the engine for `id`. `Ok(note)` means the flow completed; the caller
/// still verifies independently.
async fn run_engine(
    id: &ReauthTargetId,
    progress: &Progress,
) -> Result<Option<String>, ReauthError> {
    match id {
        ReauthTargetId::AwsSession(_) | ReauthTargetId::AwsProfile(_) => {
            aws::run(id, progress).await.map(|()| None)
        }
        ReauthTargetId::Sgsc(backend) => sgsc::run(backend, progress)
            .await
            .map(|()| Some("confirmed by the gateway's own onboarding callback".to_string())),
        ReauthTargetId::CliTool(tool) => {
            let _guard = cli_tools::try_begin_login(*tool).ok_or_else(|| {
                ReauthError::Busy("an interactive sign-in for this tool is in progress".into())
            })?;
            let plan = cli_tools::login_plan(*tool).await?;
            let mut attempt = 1;
            loop {
                let result: Result<(), CliToolError> = match &plan {
                    CliToolLoginPlan::EntraMint { client_id, scope } => {
                        cli_tools::run_entra_login(*tool, client_id, scope, progress).await
                    }
                    CliToolLoginPlan::EntraNativeBrowser { args } => {
                        cli_tools::run_entra_native_browser_login(*tool, args, progress).await
                    }
                    CliToolLoginPlan::ApiToken => {
                        return acli::run(progress).await.map(|()| None);
                    }
                    CliToolLoginPlan::Command(_) => {
                        return Err(ReauthError::NotConfigured(
                            "this tool's sign-in needs an interactive terminal".into(),
                        ));
                    }
                };
                match result {
                    Ok(()) => return Ok(None),
                    Err(CliToolError::Entra(e)) if entra_mint::should_retry(&e, attempt) => {
                        progress.say(format!("{e}; retrying with a fresh flow"));
                        attempt += 1;
                    }
                    Err(e) => return Err(e.into()),
                }
            }
        }
    }
}

/// Profiles whose shared token a scope target refreshes.
fn aws_members(id: &ReauthTargetId) -> Result<Vec<String>, ReauthError> {
    let scopes = aws_sso::profile_scopes()?;
    let members: Vec<String> = match id {
        ReauthTargetId::AwsSession(session) => scopes
            .into_iter()
            .filter(|(_, scope)| scope.session_name.as_deref() == Some(session))
            .map(|(name, _)| name)
            .collect(),
        ReauthTargetId::AwsProfile(profile) => {
            let key = scopes
                .iter()
                .find(|(name, _)| name == profile)
                .map(|(_, scope)| scope.key.clone())
                .ok_or_else(|| ReauthError::Failed(format!("AWS profile '{profile}' not found")))?;
            scopes
                .into_iter()
                .filter(|(_, scope)| scope.key == key)
                .map(|(name, _)| name)
                .collect()
        }
        _ => Vec::new(),
    };
    Ok(members)
}

/// Member profiles tried when verifying an AWS scope.
const VERIFY_PROFILES: usize = 5;

/// The independent check. Command exit is never proof of authentication.
async fn verify(id: &ReauthTargetId) -> Result<(), String> {
    match id {
        ReauthTargetId::CliTool(tool) => {
            let status = cli_tools::status(*tool).await;
            if status.auth_state == CliToolAuthState::Authenticated {
                Ok(())
            } else {
                Err(format!(
                    "the flow finished but {} still reports {:?}{}",
                    status.display_name,
                    status.auth_state,
                    status
                        .auth_message
                        .map(|m| format!(": {m}"))
                        .unwrap_or_default()
                ))
            }
        }
        ReauthTargetId::AwsSession(_) | ReauthTargetId::AwsProfile(_) => {
            let members = aws_members(id).map_err(|e| e.to_string())?;
            if members.is_empty() {
                return Err("no AWS profiles use this sign-in scope".to_string());
            }
            // Any member authenticating proves the freshly written shared
            // token (the CLI already exited 0 having stored it, so cached
            // role credentials cannot mask a failed login here). Trying a few
            // keeps one profile with a removed role assignment from failing
            // a repair that worked.
            let mut last = String::new();
            for name in members.iter().take(VERIFY_PROFILES) {
                match aws_sso::profile_status(name).await.map(|s| s.auth) {
                    Ok(aws_sso::AwsAuthStatus::Authenticated { .. }) => return Ok(()),
                    Ok(other) => last = format!("profile '{name}' reports {other:?}"),
                    Err(e) => last = format!("profile '{name}': {e}"),
                }
            }
            Err(format!(
                "the sign-in finished but no profile verified ({last})"
            ))
        }
        // The gateway's callback page is the only authority; the engine
        // already required it.
        ReauthTargetId::Sgsc(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_ids_round_trip_and_validate() {
        for wire in [
            "aws-sso:sweetgreen",
            "aws-profile:bigdata.ViewOnly",
            "cli-tool:az",
            "cli-tool:graph-powershell-1.0",
            "cli-tool:acli",
            "sgsc:dp",
        ] {
            let id: ReauthTargetId = wire.parse().unwrap();
            assert_eq!(id.to_string(), wire);
        }
        for bad in [
            "sgsc:DP",
            "sgsc:../x",
            "sgsc:",
            "aws-sso:bad name",
            "aws-profile:x]\n[default",
            "cli-tool:gam",
            "cli-tool:nope",
            "shell:rm",
            "noprefix",
        ] {
            assert!(bad.parse::<ReauthTargetId>().is_err(), "{bad:?} accepted");
        }
    }

    fn now() -> (DateTime<Utc>, Instant) {
        (Utc::now(), Instant::now())
    }

    #[test]
    fn a_running_target_is_joined_not_started_twice() {
        let mut states = HashMap::new();
        let id = ReauthTargetId::Sgsc("dp".into());
        assert!(matches!(
            claim(&mut states, &id, ReauthTrigger::Agent, Utc::now()),
            Claim::Started(_)
        ));
        match claim(&mut states, &id, ReauthTrigger::Manual, Utc::now()) {
            Claim::Joined(run) => {
                assert!(run.already_running);
                assert_eq!(run.outcome, ReauthRunOutcome::Running);
            }
            _ => panic!("second claim must join"),
        }
        finish(
            &mut states,
            &id,
            ReauthRunOutcome::Succeeded,
            None,
            now(),
            sweep::MIN_INTERVAL,
        );
        assert!(matches!(
            claim(&mut states, &id, ReauthTrigger::Agent, Utc::now()),
            Claim::Started(_)
        ));
    }

    #[test]
    fn a_refusal_blocks_agents_and_the_sweep_until_an_operator_reruns() {
        let mut states = HashMap::new();
        let id = ReauthTargetId::AwsSession("sg".into());
        claim(&mut states, &id, ReauthTrigger::Agent, Utc::now());
        let escalation = finish(
            &mut states,
            &id,
            ReauthRunOutcome::Refused,
            Some("Entra rejected the 1Password password".into()),
            now(),
            sweep::MIN_INTERVAL,
        );
        assert_eq!(escalation, Escalation::Emit, "a refusal pages at once");
        for trigger in [ReauthTrigger::Agent, ReauthTrigger::Sweep] {
            match claim(&mut states, &id, trigger, Utc::now()) {
                Claim::Blocked(run) => {
                    assert_eq!(run.outcome, ReauthRunOutcome::Refused);
                    assert!(
                        run.message
                            .unwrap()
                            .contains("rejected the 1Password password")
                    );
                }
                _ => panic!("{trigger:?} must not retry a refused target"),
            }
        }
        assert!(matches!(
            claim(&mut states, &id, ReauthTrigger::Manual, Utc::now()),
            Claim::Started(_)
        ));
    }

    #[test]
    fn sweep_failures_back_off_and_escalate_once_after_three() {
        let mut states = HashMap::new();
        let id = ReauthTargetId::CliTool(CliToolId::Az);
        let mut emitted = 0;
        for _ in 0..5 {
            claim(&mut states, &id, ReauthTrigger::Sweep, Utc::now());
            if finish(
                &mut states,
                &id,
                ReauthRunOutcome::Failed,
                Some("x".into()),
                now(),
                sweep::MIN_INTERVAL,
            ) == Escalation::Emit
            {
                emitted += 1;
            }
        }
        assert_eq!(emitted, 1, "one page per escalation, not per failure");
        let state = &states[&id];
        assert_eq!(state.consecutive_failures, 5);
        assert!(state.next_eligible.unwrap() > Instant::now());
        // Success resets, so a later breakage escalates again.
        claim(&mut states, &id, ReauthTrigger::Sweep, Utc::now());
        finish(
            &mut states,
            &id,
            ReauthRunOutcome::Succeeded,
            None,
            now(),
            sweep::MIN_INTERVAL,
        );
        let state = &states[&id];
        assert_eq!(state.consecutive_failures, 0);
        assert!(!state.escalated && state.next_eligible.is_none());
    }

    #[test]
    fn manual_and_agent_failures_do_not_consume_sweep_backoff() {
        let mut states = HashMap::new();
        let id = ReauthTargetId::CliTool(CliToolId::Az);
        claim(&mut states, &id, ReauthTrigger::Agent, Utc::now());
        let escalation = finish(
            &mut states,
            &id,
            ReauthRunOutcome::Failed,
            None,
            now(),
            sweep::MIN_INTERVAL,
        );
        assert_eq!(escalation, Escalation::None);
        assert_eq!(states[&id].consecutive_failures, 0);
    }

    #[test]
    fn transcript_lines_never_carry_query_strings() {
        assert_eq!(
            redact_line(
                "  sign-in step: Totp (https://login.microsoftonline.com/t/saml2?SAMLRequest=abc)\r\n"
            ),
            "  sign-in step: Totp (https://login.microsoftonline.com/t/saml2"
        );
        assert_eq!(
            redact_line(
                "opened https://device.sso.us-east-1.amazonaws.com/?user_code=ABCD-EFGH now"
            ),
            "opened https://device.sso.us-east-1.amazonaws.com/ now"
        );
    }

    #[test]
    fn aws_targets_resolve_to_one_canonical_id_per_token() {
        let scope = |key: &str, session: Option<&str>| aws_sso::AwsSsoAuthScope {
            key: key.into(),
            label: key.into(),
            session_name: session.map(str::to_string),
        };
        let scopes = vec![
            ("sg.Admin".to_string(), scope("session:sg", Some("sg"))),
            ("legacy.A".to_string(), scope("start-url:https://x", None)),
            ("legacy.B".to_string(), scope("start-url:https://x", None)),
            ("other".to_string(), scope("start-url:https://y", None)),
        ];
        let canon = |p: &str| canonical_aws_target(&scopes, p);
        assert_eq!(
            canon("sg.Admin"),
            Some(ReauthTargetId::AwsSession("sg".into()))
        );
        // Both legacy aliases of one start URL share the first one's id.
        assert_eq!(
            canon("legacy.A"),
            Some(ReauthTargetId::AwsProfile("legacy.A".into()))
        );
        assert_eq!(
            canon("legacy.B"),
            Some(ReauthTargetId::AwsProfile("legacy.A".into()))
        );
        assert_eq!(
            canon("other"),
            Some(ReauthTargetId::AwsProfile("other".into()))
        );
        assert_eq!(canon("missing"), None);
    }

    #[test]
    fn a_shared_entra_refusal_gates_every_non_manual_attempt() {
        let reason = "Entra rejected the 1Password password";
        let t = Utc::now();
        let earlier = EntraRefusal {
            reason: reason.into(),
            at: t - chrono::Duration::minutes(5),
        };
        for trigger in [ReauthTrigger::Agent, ReauthTrigger::Sweep] {
            let gated = entra_gate(trigger, Some(&earlier), t).expect("must be gated");
            assert!(gated.contains(reason) && gated.starts_with("not attempted"));
        }
        // An operator run requested after the refusal is the reset…
        assert_eq!(entra_gate(ReauthTrigger::Manual, Some(&earlier), t), None);
        // …but a refusal hit after it was queued (an earlier target in the
        // same batch) stops it, or one click could lock the account.
        let during_batch = EntraRefusal {
            reason: reason.into(),
            at: t + chrono::Duration::seconds(30),
        };
        assert!(entra_gate(ReauthTrigger::Manual, Some(&during_batch), t).is_some());
        assert_eq!(entra_gate(ReauthTrigger::Sweep, None, t), None);
        assert!(uses_entra(&ReauthTargetId::AwsSession("sg".into())));
        assert!(uses_entra(&ReauthTargetId::Sgsc("dp".into())));
        assert!(uses_entra(&ReauthTargetId::CliTool(CliToolId::Az)));
        assert!(!uses_entra(&ReauthTargetId::CliTool(CliToolId::Acli)));
    }

    #[test]
    fn bulk_selection_respects_refusals_and_backoff_by_trigger() {
        let now = Instant::now();
        let refused = TargetState {
            refused: true,
            ..Default::default()
        };
        assert!(!selectable(Some(&refused), ReauthTrigger::Agent, now));
        assert!(!selectable(Some(&refused), ReauthTrigger::Sweep, now));
        assert!(selectable(Some(&refused), ReauthTrigger::Manual, now));
        let backing_off = TargetState {
            next_eligible: Some(now + Duration::from_secs(600)),
            ..Default::default()
        };
        assert!(!selectable(Some(&backing_off), ReauthTrigger::Sweep, now));
        assert!(selectable(Some(&backing_off), ReauthTrigger::Agent, now));
        assert!(selectable(None, ReauthTrigger::Sweep, now));
    }

    #[test]
    fn only_authentication_errors_count_as_expired() {
        assert!(auth_failure_confirmed(
            CliToolId::Az,
            "ERROR: AADSTS700082: The refresh token has expired. Please run 'az login'."
        ));
        assert!(!auth_failure_confirmed(
            CliToolId::Az,
            "ERROR: HTTPSConnectionPool: Max retries exceeded (Name or service not known)"
        ));
        assert!(auth_failure_confirmed(
            CliToolId::Acli,
            "✗ Error: unauthorized: use 'acli confluence auth login' to authenticate"
        ));
        assert!(!auth_failure_confirmed(
            CliToolId::Acli,
            "✗ Error: page 4796448789 not found"
        ));
        assert!(auth_failure_confirmed(CliToolId::GraphPowershell10, "\n"));
        assert!(!auth_failure_confirmed(
            CliToolId::GraphPowershell10,
            "pwsh: module import failed"
        ));
        assert!(auth_failure_confirmed(
            CliToolId::GraphPowershell10,
            "WARNING: vibe-kanban: Graph auto-connect failed: {\"error\":\"invalid_grant\"}"
        ));
        assert!(!auth_failure_confirmed(
            CliToolId::GraphPowershell10,
            "WARNING: vibe-kanban: Graph auto-connect failed: No such host is known."
        ));
    }

    #[test]
    fn refusals_are_recognised_from_any_error_path() {
        let token_endpoint = ReauthError::Entra(EntraError::Auth(
            "token endpoint 400: {\"error\":\"invalid_grant\",\"error_description\":\"AADSTS53003: blocked by Conditional Access\"}".into(),
        ));
        assert!(normalize_refusal(token_endpoint).is_refusal());
        let plain = ReauthError::Failed("aws sso login failed: timeout".into());
        assert!(!normalize_refusal(plain).is_refusal());
    }

    #[test]
    fn escalation_marker_is_pinned_for_the_alert_unit() {
        // homelab/modules/vibe-kanban-reauth-alert.nix matches this text.
        assert_eq!(ESCALATION_MARKER, "vk-reauth: operator action required");
    }

    #[test]
    fn aws_scope_state_is_conservative() {
        use aws_sso::AwsAuthStatus as S;
        let ok = S::Authenticated {
            identity: "arn".into(),
        };
        let expired = S::Unauthenticated;
        let unknown = S::Unknown {
            message: "timed out".into(),
        };
        assert_eq!(aggregate_aws(&[&ok, &ok]).0, ReauthAuthState::Authenticated);
        assert_eq!(
            aggregate_aws(&[&ok, &expired, &unknown]).0,
            ReauthAuthState::Unauthenticated
        );
        assert_eq!(
            aggregate_aws(&[&ok, &unknown]),
            (ReauthAuthState::Unknown, Some("timed out".into()))
        );
        assert_eq!(
            aggregate_aws(&[&S::CliMissing]).0,
            ReauthAuthState::NotConfigured
        );
        assert_eq!(aggregate_aws(&[]).0, ReauthAuthState::Unknown);
    }
}
