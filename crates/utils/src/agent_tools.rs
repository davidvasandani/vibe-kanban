//! Whether an agent on this host can actually run a CLI tool.
//!
//! "Installed" is a fact about a directory; "usable by agents" is a fact about
//! the environment and the shells agents run commands through. The two
//! disagree in ways that matter: a login shell can replace `PATH` (NixOS
//! `/etc/set-environment`, and Codex runs every command as `bash -lc`), and a
//! wrapped tool can be present but unable to start because its runtime was
//! never supplied. This module checks the tool the way an agent reaches it —
//! through the agent environment, in both shell modes — and classifies the
//! result. It never reports a tool's authentication; that is a separate fact.
//!
//! Shared by the server's CLI Tools status and the worker's startup check, so
//! both hosts answer with the same logic.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::shell::{AGENT_PATH_ENV, UnixShell, agent_path};

/// Exit status of a vibe-kanban generated wrapper whose runtime dependency is
/// missing (`EX_UNAVAILABLE`). Distinct from any "not signed in" outcome.
pub const RUNTIME_DEPENDENCY_UNAVAILABLE_EXIT: i32 = 69;
/// Stderr prefix the generated wrappers print with that status.
pub const RUNTIME_DEPENDENCY_MESSAGE_PREFIX: &str = "vibe-kanban: runtime dependency unavailable:";
/// When set to `1`, a generated wrapper validates its runtime and exits 0
/// without launching anything.
pub const CHECK_MODE_ENV: &str = "VK_CLI_TOOL_CHECK";

const SHELL_RESOLVE_TIMEOUT: Duration = Duration::from_secs(10);
/// Long enough for a .NET tool behind dbus-run-session to print its version.
pub const AGENT_PROBE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_MESSAGE_CHARS: usize = 240;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AgentToolState {
    /// Resolves in login and non-login agent shells and starts.
    Available,
    /// Not on the agent PATH at all.
    Missing,
    /// On the agent PATH, but a login shell drops it.
    LoginShellMissing,
    /// Login and non-login shells resolve different executables, so agents
    /// run a different copy depending on how their executor spawns commands.
    LoginShellMismatch,
    /// Resolves, but its wrapper reports a missing runtime dependency.
    RuntimeUnavailable,
    /// Resolves, but did not start for another reason.
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct AgentToolCheck {
    pub state: AgentToolState,
    /// `command -v` in a non-login agent shell.
    pub path: Option<String>,
    /// `command -v` in a login agent shell.
    pub login_path: Option<String>,
    /// First line of the version probe, when it ran and succeeded.
    pub version: Option<String>,
    /// Actionable, sanitized reason when `state` is not `available`.
    pub message: Option<String>,
}

/// How a probe of the resolved command ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeOutcome {
    Succeeded { first_line: Option<String> },
    Exited { code: Option<i32>, stderr: String },
    TimedOut,
    SpawnFailed,
}

/// What to run once the tool has resolved.
#[derive(Debug, Clone, Copy)]
pub enum Probe<'a> {
    /// Run with these args and report the first stdout line as the version.
    Version(&'a [&'a str]),
    /// The tool may be reached through a vibe-kanban generated wrapper at
    /// `wrapper` (its `bin/` entry). When that is what resolves, validate its
    /// runtime first ([`CHECK_MODE_ENV`]); either way, then run with `args`.
    /// A host copy that shadows the wrapper is never run in check mode.
    Wrapper {
        wrapper: &'a Path,
        args: &'a [&'a str],
    },
    /// Validate a generated wrapper's runtime in check mode, launching
    /// nothing else. Used where the tool's version arguments are unknown.
    DependenciesOnly { wrapper: &'a Path },
    /// Only check that it resolves.
    None,
}

/// The environment an agent process on this host receives from vibe-kanban:
/// this process's environment with `PATH` and [`AGENT_PATH_ENV`] set exactly
/// as the agent environment boundaries set them.
pub fn agent_environment() -> Vec<(OsString, OsString)> {
    let path = agent_path(std::env::var_os("PATH").unwrap_or_default());
    std::env::vars_os()
        .filter(|(key, _)| key != "PATH" && key != AGENT_PATH_ENV)
        .chain([
            (OsString::from("PATH"), path.clone()),
            (OsString::from(AGENT_PATH_ENV), path),
        ])
        .collect()
}

/// How a probe reaches the tool.
#[derive(Clone, Copy)]
enum Launch<'a> {
    /// Exec the path a non-login shell resolved, with the agent environment.
    Direct(&'a str),
    /// Exec the tool by name inside a login shell, so the login profile's
    /// environment (and PATH) is what the tool runs with.
    LoginShell { shell: &'a Path, name: &'a str },
}

/// Check `binary_name` as an agent on this host would reach it.
pub async fn check(binary_name: &str, probe: Probe<'_>) -> AgentToolCheck {
    let env = agent_environment();
    if !cfg!(unix) {
        // No POSIX login shell to lose PATH in: resolve on the agent PATH the
        // way the platform's own exec does, and probe that.
        let path = resolve_on_path(binary_name, &env);
        let outcome = match path.as_deref() {
            Some(resolved) => probe_outcome(Launch::Direct(resolved), resolved, probe, &env).await,
            None => None,
        };
        return classify(binary_name, path.clone(), path, outcome, None);
    }
    let shell = agent_shell();
    let path = resolve_in_shell(&shell, false, binary_name, &env).await;
    let login_path = resolve_in_shell(&shell, true, binary_name, &env).await;
    let outcome = match path.as_deref() {
        Some(resolved) => probe_outcome(Launch::Direct(resolved), resolved, probe, &env).await,
        None => None,
    };
    // A login shell can resolve the tool yet still break it (a different copy,
    // or an interpreter its shebang finds on PATH). Run it there too, once the
    // non-login run has passed.
    let non_login_ok = matches!(outcome, None | Some(ProbeOutcome::Succeeded { .. }));
    let login_outcome = match login_path.as_deref() {
        Some(resolved) if path.is_some() && non_login_ok => {
            let launch = Launch::LoginShell {
                shell: &shell,
                name: binary_name,
            };
            probe_outcome(launch, resolved, probe, &env).await
        }
        _ => None,
    };
    classify(binary_name, path, login_path, outcome, login_outcome)
}

/// Run `probe` through `launch`. `resolved` is the path that launch will
/// reach, which decides whether it is our wrapper (and so gets check mode).
async fn probe_outcome(
    launch: Launch<'_>,
    resolved: &str,
    probe: Probe<'_>,
    env: &[(OsString, OsString)],
) -> Option<ProbeOutcome> {
    let mut outcome = None;
    if let Probe::Wrapper { wrapper, .. } | Probe::DependenciesOnly { wrapper } = probe
        && Path::new(resolved) == wrapper
    {
        outcome = Some(run(launch, &[], env, true).await);
    }
    if matches!(outcome, None | Some(ProbeOutcome::Succeeded { .. }))
        && let Probe::Version(args) | Probe::Wrapper { args, .. } = probe
    {
        outcome = Some(run(launch, args, env, false).await);
    }
    outcome
}

/// `binary_name` on the agent PATH, resolved without a shell.
fn resolve_on_path(binary_name: &str, env: &[(OsString, OsString)]) -> Option<String> {
    let path = env
        .iter()
        .find(|(k, _)| k == "PATH")
        .map(|(_, v)| v.clone());
    let cwd = std::env::current_dir().unwrap_or_default();
    which::which_in(binary_name, path, cwd)
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

/// Check every entry of the app-managed `cli-tools/bin` as this host's agents
/// would reach it. Generated wrappers that support [`CHECK_MODE_ENV`] have
/// their runtime validated; nothing is otherwise launched, so this is safe at
/// service start and touches no credential state.
pub async fn check_managed_bin() -> Vec<(String, AgentToolCheck)> {
    let bin = crate::assets::cli_tools_dir().join("bin");
    let Ok(entries) = std::fs::read_dir(&bin) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| !name.starts_with('.'))
        .collect();
    names.sort();
    let mut results = Vec::with_capacity(names.len());
    let env = agent_environment();
    for name in names {
        let entry = bin.join(&name);
        let script = std::fs::read_to_string(&entry).unwrap_or_default();
        let supports_check_mode = script.contains(CHECK_MODE_ENV);
        let probe = if supports_check_mode {
            Probe::DependenciesOnly { wrapper: &entry }
        } else {
            Probe::None
        };
        let mut result = check(&name, probe).await;
        // A wrapper written by an older build has no check mode, and is only
        // rewritten when a service serves CLI Tools status -- possibly after
        // this one-shot check. Validate the variables it requires statically
        // instead of reporting it usable unexamined.
        // Only when the wrapper is what agents reach: a host copy earlier on
        // PATH shadows it, and its dependencies are then irrelevant.
        if !supports_check_mode
            && result.path.as_deref().map(Path::new) == Some(entry.as_path())
            && let Some(message) = legacy_wrapper_unavailable(&script, &env)
        {
            result.state = AgentToolState::RuntimeUnavailable;
            result.message = Some(message);
        }
        results.push((name, result));
    }
    results
}

/// For a vibe-kanban generated wrapper that predates [`CHECK_MODE_ENV`]: the
/// first variable its `: "${VAR:?…}"` guards require that the agent
/// environment does not provide, as a sanitized message. The value is never
/// included, only the name.
fn legacy_wrapper_unavailable(script: &str, env: &[(OsString, OsString)]) -> Option<String> {
    if !script.contains("# Generated by vibe-kanban") {
        return None;
    }
    let guards = regex::Regex::new(r#"\$\{(VK_[A-Z0-9_]+):\?"#).expect("static regex");
    guards.captures_iter(script).find_map(|capture| {
        let name = &capture[1];
        let set = env
            .iter()
            .any(|(key, value)| key == name && !value.is_empty() && Path::new(value).exists());
        (!set).then(|| {
            format!(
                "runtime dependency unavailable: {name} (not set or missing); the service \
                 environment on this host must supply it"
            )
        })
    })
}

/// The shell agents run commands through. Executors use the account's shell;
/// systemd units usually leave `SHELL` unset, so fall back to bash (what Codex
/// uses) before `/bin/sh`. The probes are POSIX scripts, so a non-POSIX
/// `SHELL` (fish, nu, …) falls back too rather than failing every check.
fn agent_shell() -> PathBuf {
    if let Ok(shell) = std::env::var("SHELL")
        && let Some(UnixShell::Bash(shell) | UnixShell::Zsh(shell) | UnixShell::Sh(shell)) =
            UnixShell::from_path(Path::new(&shell))
    {
        return shell;
    }
    which::which("bash").unwrap_or_else(|_| PathBuf::from("/bin/sh"))
}

async fn resolve_in_shell(
    shell: &Path,
    login: bool,
    binary_name: &str,
    env: &[(OsString, OsString)],
) -> Option<String> {
    let mut command = tokio::process::Command::new(shell);
    if login {
        command.arg("-l");
    }
    command
        // The name travels as a positional argument, never inside the script.
        .args(["-c", r#"command -v "$1""#, "vk-agent-check", binary_name])
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(SHELL_RESOLVE_TIMEOUT, command.output())
        .await
        .ok()?
        .ok()?;
    if !output.status.success() {
        return None;
    }
    // Login profiles may print banners; `command -v` prints the path last.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().rev().find(|l| !l.trim().is_empty())?.trim();
    // Aliases, functions and builtins are not what an agent's exec would get.
    Path::new(line).is_absolute().then(|| line.to_string())
}

async fn run(
    launch: Launch<'_>,
    args: &[&str],
    env: &[(OsString, OsString)],
    check_mode: bool,
) -> ProbeOutcome {
    let mut command = match launch {
        Launch::Direct(resolved) => tokio::process::Command::new(resolved),
        Launch::LoginShell { shell, name } => {
            let mut command = tokio::process::Command::new(shell);
            // Name and args travel as positional arguments, never as script.
            command.args([
                "-l",
                "-c",
                r#"n=$1; shift; exec "$n" "$@""#,
                "vk-agent-check",
                name,
            ]);
            command
        }
    };
    command
        .args(args)
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if check_mode {
        command.env(CHECK_MODE_ENV, "1");
    }
    match tokio::time::timeout(AGENT_PROBE_TIMEOUT, command.output()).await {
        Err(_) => ProbeOutcome::TimedOut,
        Ok(Err(_)) => ProbeOutcome::SpawnFailed,
        Ok(Ok(output)) if output.status.success() => ProbeOutcome::Succeeded {
            first_line: String::from_utf8_lossy(&output.stdout)
                .lines()
                .find(|l| !l.trim().is_empty())
                .map(|l| truncate(l.trim(), 120)),
        },
        Ok(Ok(output)) => ProbeOutcome::Exited {
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        },
    }
}

/// The runtime-dependency message from a generated wrapper's stderr, if the
/// failure is one. Only our own prefixed line is ever surfaced: arbitrary tool
/// stderr can carry account names, tenant ids or token fragments.
pub fn runtime_dependency_message(code: Option<i32>, stderr: &str) -> Option<String> {
    if code != Some(RUNTIME_DEPENDENCY_UNAVAILABLE_EXIT) {
        return None;
    }
    stderr
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with(RUNTIME_DEPENDENCY_MESSAGE_PREFIX))
        .map(|l| truncate(l.trim_start_matches("vibe-kanban: "), MAX_MESSAGE_CHARS))
}

/// Pure classification of what [`check`] observed: `outcome` from the
/// non-login run, `login_outcome` from the same probe inside a login shell.
pub fn classify(
    binary_name: &str,
    path: Option<String>,
    login_path: Option<String>,
    outcome: Option<ProbeOutcome>,
    login_outcome: Option<ProbeOutcome>,
) -> AgentToolCheck {
    let result = |state, version, message: Option<String>| AgentToolCheck {
        state,
        path: path.clone(),
        login_path: login_path.clone(),
        version,
        message,
    };
    if path.is_none() {
        return result(
            AgentToolState::Missing,
            None,
            Some(format!(
                "`{binary_name}` is not on the agent PATH on this host"
            )),
        );
    }
    let version = match &outcome {
        Some(ProbeOutcome::Succeeded { first_line }) => first_line.clone(),
        _ => None,
    };
    if let Some((state, message)) = failure(binary_name, outcome.as_ref(), "") {
        return result(state, None, Some(message));
    }
    if login_path.is_none() {
        return result(
            AgentToolState::LoginShellMissing,
            version,
            Some(format!(
                "`{binary_name}` resolves in non-login shells but a login shell (for example \
                 Codex's `bash -lc`) drops it; the host login profile must restore \
                 ${AGENT_PATH_ENV}"
            )),
        );
    }
    if let Some((state, message)) =
        failure(binary_name, login_outcome.as_ref(), " in a login shell")
    {
        return result(state, version, Some(message));
    }
    if login_path != path {
        return result(
            AgentToolState::LoginShellMismatch,
            version,
            Some(format!(
                "a login shell runs a different `{binary_name}` than a non-login shell; the host \
                 login profile must restore ${AGENT_PATH_ENV} first"
            )),
        );
    }
    result(AgentToolState::Available, version, None)
}

/// The state and sanitized message for a failed probe, or `None` if it did
/// not fail (or did not run). `context` names the shell mode it ran in.
fn failure(
    binary_name: &str,
    outcome: Option<&ProbeOutcome>,
    context: &str,
) -> Option<(AgentToolState, String)> {
    match outcome? {
        ProbeOutcome::Succeeded { .. } => None,
        ProbeOutcome::Exited { code, stderr } => {
            if let Some(message) = runtime_dependency_message(*code, stderr) {
                return Some((
                    AgentToolState::RuntimeUnavailable,
                    format!(
                        "{message}{context}; the service environment on this host must supply it"
                    ),
                ));
            }
            let status = code.map_or_else(|| "a signal".to_string(), |c| format!("status {c}"));
            Some((
                AgentToolState::Failed,
                format!("`{binary_name}` resolved but exited with {status}{context}"),
            ))
        }
        ProbeOutcome::TimedOut => Some((
            AgentToolState::Failed,
            format!("`{binary_name}` resolved but did not answer in time{context}"),
        )),
        ProbeOutcome::SpawnFailed => Some((
            AgentToolState::Failed,
            format!("`{binary_name}` resolved but could not be started{context}"),
        )),
    }
}

fn truncate(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

#[cfg(all(test, unix))]
mod tests {
    use std::{fs, os::unix::fs::PermissionsExt};

    use tempfile::TempDir;

    use super::*;

    fn some(s: &str) -> Option<String> {
        Some(s.to_string())
    }

    #[test]
    fn missing_from_the_agent_path_is_missing_regardless_of_probe() {
        let check = classify("az", None, None, None, None);
        assert_eq!(check.state, AgentToolState::Missing);
        assert!(check.message.unwrap().contains("`az`"));
    }

    #[test]
    fn a_login_shell_that_drops_the_tool_is_its_own_state() {
        let check = classify(
            "az",
            some("/unit/bin/az"),
            None,
            Some(ProbeOutcome::Succeeded {
                first_line: some("2.86.0"),
            }),
            None,
        );
        assert_eq!(check.state, AgentToolState::LoginShellMissing);
        assert_eq!(check.version.as_deref(), Some("2.86.0"));
        assert!(check.message.unwrap().contains(AGENT_PATH_ENV));
    }

    #[test]
    fn wrapper_runtime_failure_is_not_a_generic_failure() {
        let stderr = format!(
            "noise\n{RUNTIME_DEPENDENCY_MESSAGE_PREFIX} VK_ENTRA_LIBSECRET_LIB (not set)\n"
        );
        let check = classify(
            "mgc-beta",
            some("/tools/bin/mgc-beta"),
            some("/tools/bin/mgc-beta"),
            Some(ProbeOutcome::Exited {
                code: Some(RUNTIME_DEPENDENCY_UNAVAILABLE_EXIT),
                stderr,
            }),
            None,
        );
        assert_eq!(check.state, AgentToolState::RuntimeUnavailable);
        let message = check.message.unwrap();
        assert!(message.starts_with("runtime dependency unavailable: VK_ENTRA_LIBSECRET_LIB"));
        assert!(!message.contains("noise"));
    }

    #[test]
    fn exit_69_without_our_prefix_is_not_trusted_as_a_runtime_message() {
        let check = classify(
            "tool",
            some("/bin/tool"),
            some("/bin/tool"),
            Some(ProbeOutcome::Exited {
                code: Some(RUNTIME_DEPENDENCY_UNAVAILABLE_EXIT),
                stderr: "token=abc123".into(),
            }),
            None,
        );
        assert_eq!(check.state, AgentToolState::Failed);
        assert!(!check.message.unwrap().contains("abc123"));
    }

    #[test]
    fn resolving_in_both_modes_and_starting_is_available() {
        let check = classify(
            "az",
            some("/unit/bin/az"),
            some("/unit/bin/az"),
            Some(ProbeOutcome::Succeeded { first_line: None }),
            Some(ProbeOutcome::Succeeded { first_line: None }),
        );
        assert_eq!(check.state, AgentToolState::Available);
        assert_eq!(check.message, None);
    }

    #[test]
    fn a_legacy_wrapper_is_checked_against_the_variables_it_requires() {
        let temp = TempDir::new().unwrap();
        // Shape of the wrapper earlier builds wrote: no check mode.
        let script = "#!/bin/sh\n# Generated by vibe-kanban; do not edit.\n\
            : \"${VK_ENTRA_DBUS_RUN_SESSION:?vibe-kanban: VK_ENTRA_DBUS_RUN_SESSION is not set}\"\n\
            : \"${VK_ENTRA_LIBSECRET_LIB:?vibe-kanban: VK_ENTRA_LIBSECRET_LIB is not set}\"\n";
        let present = temp.path().as_os_str().to_os_string();
        let dbus = (OsString::from("VK_ENTRA_DBUS_RUN_SESSION"), present.clone());

        let message = legacy_wrapper_unavailable(script, std::slice::from_ref(&dbus)).unwrap();
        assert!(message.contains("VK_ENTRA_LIBSECRET_LIB"));
        assert!(!message.contains(&*temp.path().to_string_lossy()));

        let stale = (
            OsString::from("VK_ENTRA_LIBSECRET_LIB"),
            OsString::from("/nix/store/gone-libsecret/lib"),
        );
        assert!(legacy_wrapper_unavailable(script, &[dbus.clone(), stale]).is_some());

        let lib = (OsString::from("VK_ENTRA_LIBSECRET_LIB"), present);
        assert_eq!(legacy_wrapper_unavailable(script, &[dbus, lib]), None);
        // Anything that is not one of our wrappers is left alone.
        assert_eq!(
            legacy_wrapper_unavailable("#!/bin/sh\n: \"${VK_X:?}\"\n", &[]),
            None
        );
    }

    #[test]
    fn a_login_shell_that_runs_a_different_copy_is_a_mismatch() {
        let ok = || Some(ProbeOutcome::Succeeded { first_line: None });
        let check = classify(
            "az",
            some("/unit/bin/az"),
            some("/run/current-system/sw/bin/az"),
            ok(),
            ok(),
        );
        assert_eq!(check.state, AgentToolState::LoginShellMismatch);
        assert_eq!(
            check.login_path.as_deref(),
            Some("/run/current-system/sw/bin/az")
        );
        assert!(check.message.unwrap().contains(AGENT_PATH_ENV));
    }

    #[test]
    fn a_tool_that_breaks_only_in_a_login_shell_is_not_available() {
        // Resolves in both modes, starts non-login, fails under `bash -l`
        // (for example its shebang interpreter left PATH).
        let check = classify(
            "az",
            some("/unit/bin/az"),
            some("/unit/bin/az"),
            Some(ProbeOutcome::Succeeded {
                first_line: some("2.86.0"),
            }),
            Some(ProbeOutcome::Exited {
                code: Some(127),
                stderr: "env: 'python3': No such file or directory".into(),
            }),
        );
        assert_eq!(check.state, AgentToolState::Failed);
        let message = check.message.unwrap();
        assert!(
            message.ends_with("status 127 in a login shell"),
            "{message}"
        );
        assert!(!message.contains("python3"));
    }

    #[test]
    fn a_login_shell_runtime_failure_is_runtime_unavailable() {
        let check = classify(
            "mgc-beta",
            some("/tools/bin/mgc-beta"),
            some("/tools/bin/mgc-beta"),
            Some(ProbeOutcome::Succeeded { first_line: None }),
            Some(ProbeOutcome::Exited {
                code: Some(RUNTIME_DEPENDENCY_UNAVAILABLE_EXIT),
                stderr: format!("{RUNTIME_DEPENDENCY_MESSAGE_PREFIX} VK_X (not set)"),
            }),
        );
        assert_eq!(check.state, AgentToolState::RuntimeUnavailable);
        assert!(check.message.unwrap().contains("in a login shell"));
    }

    #[tokio::test]
    async fn login_shell_probe_execs_the_tool_by_name_with_its_args() {
        let _serial = SPAWN_LOCK.lock().await;
        let temp = TempDir::new().unwrap();
        write_script(temp.path(), "fake-tool", r#"printf '%s|' "$@"; exit 3"#);
        // Stand-in login shell: drops `-l`, then behaves as sh -c.
        let shell = write_script(temp.path(), "fake-login-sh", r#"shift; exec /bin/sh "$@""#);
        let env = env_with_path(temp.path());
        let outcome = run(
            Launch::LoginShell {
                shell: &shell,
                name: "fake-tool",
            },
            &["--version", "a b"],
            &env,
            false,
        )
        .await;
        assert_eq!(
            outcome,
            ProbeOutcome::Exited {
                code: Some(3),
                stderr: String::new(),
            }
        );
    }

    /// Tests that write a script and then exec it run one at a time: a fork
    /// in a concurrent test can briefly hold the new file open for writing,
    /// and exec then fails with ETXTBSY.
    static SPAWN_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    fn write_script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn env_with_path(path: &Path) -> Vec<(OsString, OsString)> {
        vec![
            (OsString::from("PATH"), path.as_os_str().to_os_string()),
            (OsString::from("HOME"), path.as_os_str().to_os_string()),
        ]
    }

    #[tokio::test]
    async fn shell_resolution_returns_absolute_paths_only() {
        let _serial = SPAWN_LOCK.lock().await;
        let temp = TempDir::new().unwrap();
        let tool = write_script(temp.path(), "fake-tool", "exit 0");
        let env = env_with_path(temp.path());
        let resolved = resolve_in_shell(Path::new("/bin/sh"), false, "fake-tool", &env).await;
        assert_eq!(resolved.as_deref(), tool.to_str());
        // A builtin resolves to its name, not a path an agent could exec.
        assert_eq!(
            resolve_in_shell(Path::new("/bin/sh"), false, "cd", &env).await,
            None
        );
        assert_eq!(
            resolve_in_shell(Path::new("/bin/sh"), false, "absent-tool", &env).await,
            None
        );
    }

    #[tokio::test]
    async fn check_mode_runs_before_the_version_probe() {
        let _serial = SPAWN_LOCK.lock().await;
        let temp = TempDir::new().unwrap();
        let tool = write_script(
            temp.path(),
            "wrapped",
            &format!(
                r#"if [ "${CHECK_MODE_ENV}" = 1 ]; then echo "{RUNTIME_DEPENDENCY_MESSAGE_PREFIX} VK_X (not set)" >&2; exit 69; fi
echo should-not-run"#
            ),
        );
        let env = env_with_path(temp.path());
        let outcome = run(Launch::Direct(tool.to_str().unwrap()), &[], &env, true).await;
        let ProbeOutcome::Exited { code, stderr } = outcome else {
            panic!("expected an exit, got {outcome:?}");
        };
        assert_eq!(code, Some(RUNTIME_DEPENDENCY_UNAVAILABLE_EXIT));
        assert!(runtime_dependency_message(code, &stderr).is_some());
    }

    #[test]
    fn agent_environment_carries_identical_path_and_agent_path() {
        let env = agent_environment();
        let path = env.iter().find(|(k, _)| k == "PATH").map(|(_, v)| v);
        let agent = env
            .iter()
            .find(|(k, _)| k == AGENT_PATH_ENV)
            .map(|(_, v)| v);
        assert!(path.is_some());
        assert_eq!(path, agent);
        assert_eq!(env.iter().filter(|(k, _)| k == "PATH").count(), 1);
    }
}
