//! Resolve stored 1Password references only while preparing a child environment.
//! Never persist resolved values or include provider output in diagnostics.
//!
//! An organization may configure 1Password Connect (`OP_CONNECT_HOST` plus a
//! literal `OP_CONNECT_TOKEN`). Connect is then tried first, because it serves
//! from a local cache and does not spend the service account's request quota;
//! the service-account token is the fallback. Connect credentials come only
//! from the organization, never from the service's ambient environment, so a
//! reference resolves only with credentials the organization itself supplied.

use std::{
    collections::HashMap,
    path::Path,
    process::Stdio,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use api_types::normalize_secret_reference;
use thiserror::Error;
use tokio::{io::AsyncReadExt, process::Command};

use super::cli_tools::{CliToolId, effective_binary_for};

const TOKEN: &str = "OP_SERVICE_ACCOUNT_TOKEN";
const CONNECT_HOST: &str = "OP_CONNECT_HOST";
const CONNECT_TOKEN: &str = "OP_CONNECT_TOKEN";
/// Organization Env Vars that select the 1Password identity. Callers that
/// resolve a single reference on an organization's behalf copy these.
pub const CREDENTIAL_VARIABLES: [&str; 3] = [TOKEN, CONNECT_HOST, CONNECT_TOKEN];
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(30);
/// Bounds one Connect attempt, as the host `op-read` wrapper does: a wedged
/// Connect server can accept the connection and never answer.
const CONNECT_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(10);
/// How long a Connect host that timed out is skipped.
const CONNECT_COOLDOWN: Duration = Duration::from_secs(60);

static CONNECT_BREAKER: LazyLock<ConnectBreaker> =
    LazyLock::new(|| ConnectBreaker::new(CONNECT_ATTEMPT_TIMEOUT, CONNECT_COOLDOWN));

/// Remembers Connect hosts that recently timed out. One launch resolves Env
/// Vars and each GitHub owner token in separate calls, and launches overlap,
/// so the memory is process-wide: until the cooldown passes, every lookup goes
/// straight to the service account instead of waiting on Connect again.
struct ConnectBreaker {
    attempt_timeout: Duration,
    cooldown: Duration,
    silent_until: Mutex<HashMap<String, Instant>>,
}

impl ConnectBreaker {
    fn new(attempt_timeout: Duration, cooldown: Duration) -> Self {
        Self {
            attempt_timeout,
            cooldown,
            silent_until: Mutex::new(HashMap::new()),
        }
    }

    fn is_silent(&self, host: &str) -> bool {
        let mut silent = self
            .silent_until
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let now = Instant::now();
        silent.retain(|_, until| *until > now);
        silent.contains_key(host)
    }

    fn mark_silent(&self, host: &str) {
        self.silent_until
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(host.to_owned(), Instant::now() + self.cooldown);
    }
}
const MAX_SECRET_BYTES: u64 = 64 * 1024;
/// Enough for any `op` error line; read only to classify, then dropped.
const MAX_DIAGNOSTIC_BYTES: u64 = 8 * 1024;

/// Why `op read` failed, derived from its stderr. The stderr itself echoes the
/// reference and may change between CLI versions, so it is never surfaced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LookupFailure {
    RateLimited,
    Unauthorized,
    NotFound,
    Unknown,
}

impl std::fmt::Display for LookupFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::RateLimited => {
                "1Password rate-limited the request (the service account's request quota is used up); retry after the quota resets — `op service-account ratelimit` shows when"
            }
            Self::Unauthorized => {
                "1Password rejected the token; check that it is current and not revoked"
            }
            Self::NotFound => {
                "the vault, item, or field in the reference was not found or is not shared with this credential"
            }
            Self::Unknown => "check the reference, token, vault permissions, and connectivity",
        })
    }
}

/// `op` echoes the reference, vault, item and field names into its errors,
/// and a name such as `token429` must not decide the cause. So quoted
/// segments are removed, `op`'s own not-found phrasing is checked first (it
/// only follows a successful, authorized request), and statuses must appear
/// in the `(NNN)` form `op` prints them in.
fn classify(stderr: &[u8]) -> LookupFailure {
    let text = without_quoted(&String::from_utf8_lossy(stderr).to_lowercase());
    let any = |needles: &[&str]| needles.iter().any(|needle| text.contains(needle));
    if any(&[
        "isn't an item",
        "isn't a vault",
        "isn't a field",
        "does not have a field",
    ]) {
        LookupFailure::NotFound
    } else if any(&[
        "rate limit",
        "rate-limit",
        "ratelimit",
        "too many requests",
        "(429)",
    ]) {
        LookupFailure::RateLimited
    } else if any(&[
        "unauthorized",
        "(401)",
        "invalid token",
        "decodesacredentials",
    ]) {
        LookupFailure::Unauthorized
    } else if any(&["forbidden", "(403)", "(404)", "not found"]) {
        LookupFailure::NotFound
    } else {
        LookupFailure::Unknown
    }
}

/// Drops text inside matching single or double quotes, where `op` echoes
/// user-chosen names. An apostrophe inside a word (`isn't`, `David's`) neither
/// opens nor closes a quote, and an unmatched quote keeps the rest of the text.
fn without_quoted(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let word = |index: Option<usize>| {
        index
            .and_then(|i| chars.get(i))
            .is_some_and(|c| c.is_alphanumeric())
    };
    let is_quote = |i: usize, opening: bool| match chars[i] {
        '"' => true,
        '\'' if opening => !word(i.checked_sub(1)),
        '\'' => !word(Some(i + 1)),
        _ => false,
    };
    let mut kept = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        if is_quote(i, true)
            && let Some(end) =
                (i + 1..chars.len()).find(|&j| chars[j] == chars[i] && is_quote(j, false))
        {
            kept.push(' ');
            i = end + 1;
            continue;
        }
        kept.push(chars[i]);
        i += 1;
    }
    kept
}

/// Which credential a failed lookup used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    ServiceAccount,
    Connect,
}

impl std::fmt::Display for Via {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::ServiceAccount => "the service account (OP_SERVICE_ACCOUNT_TOKEN)",
            Self::Connect => "1Password Connect (OP_CONNECT_HOST)",
        })
    }
}

#[derive(Debug, Error)]
pub enum EnvironmentSecretError {
    #[error(
        "1Password references require a nonempty literal OP_SERVICE_ACCOUNT_TOKEN, or OP_CONNECT_HOST and OP_CONNECT_TOKEN, in organization Env Vars, or OP_SERVICE_ACCOUNT_TOKEN in the Vibe Kanban service environment"
    )]
    MissingToken,
    #[error(
        "1Password Connect needs both OP_CONNECT_HOST and OP_CONNECT_TOKEN as nonempty literal organization Env Vars"
    )]
    IncompleteConnect,
    #[error(
        "1Password CLI is unavailable; install it in CLI Tools or on the Vibe Kanban service PATH"
    )]
    MissingCli,
    #[error(
        "1Password lookup{} failed through {via}: {cause}",
        variable.as_ref().map(|name| format!(" for {name}")).unwrap_or_default()
    )]
    ReadFailed {
        /// Env var name (never its value or reference); `None` when the caller
        /// names the input itself.
        variable: Option<String>,
        via: Via,
        cause: LookupFailure,
    },
    #[error("1Password lookup timed out; check service connectivity and retry")]
    TimedOut,
    #[error("1Password field must be NUL-free UTF-8 text of at most 64 KiB")]
    InvalidValue,
    #[error(
        "the 1Password credential file named by {variable} is unreadable or is not one raw line of text"
    )]
    CredentialFile { variable: &'static str },
}

impl EnvironmentSecretError {
    fn read_failed(via: Via, cause: LookupFailure) -> Self {
        Self::ReadFailed {
            variable: None,
            via,
            cause,
        }
    }

    /// For single-reference callers whose map key is an internal placeholder.
    pub fn without_variable(self) -> Self {
        match self {
            Self::ReadFailed { via, cause, .. } => Self::read_failed(via, cause),
            other => other,
        }
    }
}

/// The only `OP_*` settings one `op read` child receives.
enum Credential<'a> {
    ServiceAccount(&'a str),
    Connect { host: &'a str, token: &'a str },
}

impl Credential<'_> {
    fn via(&self) -> Via {
        match self {
            Self::ServiceAccount(_) => Via::ServiceAccount,
            Self::Connect { .. } => Via::Connect,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Credentials {
    connect: Option<(String, String)>,
    service_account: Option<String>,
}

/// Names a file holding the Connect token for the server's own lookups; it
/// pairs with the ambient `OP_CONNECT_HOST`.
pub const SERVER_CONNECT_TOKEN_FILE: &str = "VK_SERVER_OP_CONNECT_TOKEN_FILE";
/// Names a file holding a service-account token for the server's own lookups.
pub const SERVER_SERVICE_ACCOUNT_TOKEN_FILE: &str = "VK_SERVER_OP_SERVICE_ACCOUNT_TOKEN_FILE";
const MAX_CREDENTIAL_FILE_BYTES: u64 = 16 * 1024;

/// 1Password credentials the deployment supplies for the server's *own*
/// lookups (Settings → GitHub organization tokens), as explicit entries for a
/// single-reference request. They arrive as file paths, never values, so the
/// service can keep `OP_*` tokens out of the environment its agents inherit.
/// Empty when neither variable is set.
pub fn server_credentials() -> Result<HashMap<String, String>, EnvironmentSecretError> {
    server_credentials_from(|name| std::env::var(name).ok())
}

fn server_credentials_from(
    env: impl Fn(&str) -> Option<String>,
) -> Result<HashMap<String, String>, EnvironmentSecretError> {
    let path = |name: &str| env(name).filter(|value| !value.trim().is_empty());
    let mut credentials = HashMap::new();
    if let Some(file) = path(SERVER_CONNECT_TOKEN_FILE) {
        let token = read_credential_file(Path::new(&file)).ok_or(
            EnvironmentSecretError::CredentialFile {
                variable: SERVER_CONNECT_TOKEN_FILE,
            },
        )?;
        let host = env(CONNECT_HOST)
            .filter(|host| usable_literal(host))
            .ok_or(EnvironmentSecretError::IncompleteConnect)?;
        credentials.insert(CONNECT_HOST.to_owned(), host);
        credentials.insert(CONNECT_TOKEN.to_owned(), token);
    }
    if let Some(file) = path(SERVER_SERVICE_ACCOUNT_TOKEN_FILE) {
        let token = read_credential_file(Path::new(&file)).ok_or(
            EnvironmentSecretError::CredentialFile {
                variable: SERVER_SERVICE_ACCOUNT_TOKEN_FILE,
            },
        )?;
        credentials.insert(TOKEN.to_owned(), token);
    }
    Ok(credentials)
}

/// One raw line, the format `op-connect-token-read` accepts: a single
/// trailing newline is allowed; surrounding whitespace, inner line breaks,
/// NUL, and `NAME=value` assignment syntax are not.
fn read_credential_file(path: &Path) -> Option<String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(MAX_CREDENTIAL_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_CREDENTIAL_FILE_BYTES {
        return None;
    }
    let text = String::from_utf8(bytes).ok()?;
    let line = text.strip_suffix('\n').unwrap_or(&text);
    let assignment = line
        .split_once('=')
        .is_some_and(|(name, _)| name.starts_with("OP_"));
    (!line.is_empty() && line.trim() == line && !line.contains(['\n', '\r', '\0']) && !assignment)
        .then(|| line.to_owned())
}

/// Literal-only environments do not require a token or an installed CLI.
/// Return a new map only after every reference succeeds; callers must not spawn
/// a child on error. The supplied map is never modified or logged.
pub async fn resolve_environment_secrets(
    values: HashMap<String, String>,
) -> Result<HashMap<String, String>, EnvironmentSecretError> {
    let fallback = std::env::var(TOKEN).ok();
    resolve_with_provider(
        values,
        fallback.as_deref(),
        effective_binary_for(CliToolId::Op),
    )
    .await
}

async fn resolve_with_provider(
    values: HashMap<String, String>,
    fallback: Option<&str>,
    binary: impl std::future::Future<Output = Option<std::path::PathBuf>>,
) -> Result<HashMap<String, String>, EnvironmentSecretError> {
    if !has_references(&values) {
        return Ok(values);
    }
    let credentials = select_credentials(&values, fallback)?;
    tokio::time::timeout(RESOLVE_TIMEOUT, async {
        let binary = binary.await.ok_or(EnvironmentSecretError::MissingCli)?;
        resolve_with_binary(values, &credentials, &binary, &CONNECT_BREAKER).await
    })
    .await
    .map_err(|_| EnvironmentSecretError::TimedOut)?
}

fn has_references(values: &HashMap<String, String>) -> bool {
    values
        .values()
        .any(|value| normalize_secret_reference(value).is_some())
}

fn usable_literal(value: &str) -> bool {
    !value.trim().is_empty() && normalize_secret_reference(value).is_none() && !value.contains('\0')
}

/// An explicitly configured but unusable credential is an error, never a
/// silent fallback to another identity.
fn select_credentials(
    values: &HashMap<String, String>,
    fallback: Option<&str>,
) -> Result<Credentials, EnvironmentSecretError> {
    let configured = |name: &str| {
        values
            .get(name)
            .filter(|value| !value.trim().is_empty())
            .map(String::as_str)
    };
    let connect = match (configured(CONNECT_HOST), configured(CONNECT_TOKEN)) {
        (None, None) => None,
        (Some(host), Some(token)) if usable_literal(host) && usable_literal(token) => {
            Some((host.to_owned(), token.to_owned()))
        }
        _ => return Err(EnvironmentSecretError::IncompleteConnect),
    };
    let service_account = match values.get(TOKEN).map(String::as_str).or(fallback) {
        Some(token) if usable_literal(token) => Some(token.to_owned()),
        Some(_) if values.contains_key(TOKEN) || connect.is_none() => {
            return Err(EnvironmentSecretError::MissingToken);
        }
        _ => None,
    };
    if connect.is_none() && service_account.is_none() {
        return Err(EnvironmentSecretError::MissingToken);
    }
    Ok(Credentials {
        connect,
        service_account,
    })
}

async fn resolve_with_binary(
    mut values: HashMap<String, String>,
    credentials: &Credentials,
    binary: &Path,
    breaker: &ConnectBreaker,
) -> Result<HashMap<String, String>, EnvironmentSecretError> {
    for (name, value) in values.iter_mut() {
        // Values saved before quote normalization may still carry the quotes
        // 1Password adds when copying a reference; read the bare reference.
        if let Some(reference) = normalize_secret_reference(value) {
            *value = read_with_credentials(binary, credentials, &reference, name, breaker)
                .await
                .map_err(|error| match error {
                    EnvironmentSecretError::ReadFailed { via, cause, .. } => {
                        tracing::warn!(variable = %name, ?via, ?cause, "1Password lookup failed");
                        EnvironmentSecretError::ReadFailed {
                            variable: Some(name.clone()),
                            via,
                            cause,
                        }
                    }
                    other => other,
                })?;
        }
    }
    Ok(values)
}

async fn read_with_credentials(
    binary: &Path,
    credentials: &Credentials,
    reference: &str,
    name: &str,
    breaker: &ConnectBreaker,
) -> Result<String, EnvironmentSecretError> {
    let mut timed_out = false;
    let connect_failure = match &credentials.connect {
        Some((host, token)) if !breaker.is_silent(host) => {
            let connect = Credential::Connect { host, token };
            match tokio::time::timeout(
                breaker.attempt_timeout,
                read_reference(binary, &connect, reference),
            )
            .await
            {
                Ok(Ok(value)) => return Ok(value),
                // The field itself is unusable; the service account would
                // return the same bytes.
                Ok(Err(EnvironmentSecretError::InvalidValue)) => {
                    return Err(EnvironmentSecretError::InvalidValue);
                }
                Ok(Err(error)) => Some(error),
                Err(_) => {
                    timed_out = true;
                    breaker.mark_silent(host);
                    Some(EnvironmentSecretError::read_failed(
                        Via::Connect,
                        LookupFailure::Unknown,
                    ))
                }
            }
        }
        Some(_) => Some(EnvironmentSecretError::read_failed(
            Via::Connect,
            LookupFailure::Unknown,
        )),
        None => None,
    };
    match (&credentials.service_account, connect_failure) {
        (Some(token), failure) => {
            if let Some(EnvironmentSecretError::ReadFailed { cause, .. }) = failure {
                tracing::warn!(
                    variable = %name,
                    ?cause,
                    timed_out,
                    "1Password Connect lookup failed; falling back to the service account"
                );
            }
            read_reference(binary, &Credential::ServiceAccount(token), reference).await
        }
        (None, Some(failure)) => Err(failure),
        (None, None) => Err(EnvironmentSecretError::MissingToken),
    }
}

fn read_command(binary: &Path, credential: &Credential<'_>, reference: &str) -> Command {
    let mut command = Command::new(binary);
    // Ambient Connect credentials, account selectors, interactive sessions, and
    // OP_DEBUG must not change the selected identity or logging: the child gets
    // exactly the credential chosen from the organization's settings.
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("OP_") {
            command.env_remove(key);
        }
    }
    match credential {
        Credential::ServiceAccount(token) => {
            command.env(TOKEN, token);
        }
        Credential::Connect { host, token } => {
            command.env(CONNECT_HOST, host).env(CONNECT_TOKEN, token);
        }
    }
    command
        .args(["read", "--no-newline", "--", reference])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    command
}

async fn read_reference(
    binary: &Path,
    credential: &Credential<'_>,
    reference: &str,
) -> Result<String, EnvironmentSecretError> {
    let via = credential.via();
    let unknown = || EnvironmentSecretError::read_failed(via, LookupFailure::Unknown);
    let mut child = read_command(binary, credential, reference)
        .spawn()
        .map_err(|_| unknown())?;
    let (stdout, stderr) = child
        .stdout
        .take()
        .zip(child.stderr.take())
        .ok_or_else(unknown)?;
    // Read both pipes together so a full stderr pipe cannot stall stdout. Each
    // future owns its pipe and closes it at its bound, so a child writing past
    // either bound gets a broken pipe instead of blocking the other read.
    let mut bytes = Vec::new();
    let mut diagnostics = Vec::new();
    let (output, diagnostics_read) = (&mut bytes, &mut diagnostics);
    let (read, _) = tokio::join!(
        async move { stdout.take(MAX_SECRET_BYTES + 1).read_to_end(output).await },
        async move {
            stderr
                .take(MAX_DIAGNOSTIC_BYTES)
                .read_to_end(diagnostics_read)
                .await
        },
    );
    read.map_err(|_| unknown())?;
    if bytes.len() as u64 > MAX_SECRET_BYTES {
        return Err(EnvironmentSecretError::InvalidValue);
    }
    if !child.wait().await.map_err(|_| unknown())?.success() {
        return Err(EnvironmentSecretError::read_failed(
            via,
            classify(&diagnostics),
        ));
    }
    if bytes.contains(&0) {
        return Err(EnvironmentSecretError::InvalidValue);
    }
    String::from_utf8(bytes).map_err(|_| EnvironmentSecretError::InvalidValue)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credential_file(contents: &[u8]) -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credential");
        std::fs::write(&path, contents).unwrap();
        (dir, path.to_string_lossy().into_owned())
    }

    #[test]
    fn credential_files_accept_one_raw_line_only() {
        for (contents, expected) in [
            (&b"ops_abc=="[..], Some("ops_abc==")),
            (b"ops_abc\n", Some("ops_abc")),
            (b"", None),
            (b"\n", None),
            (b" ops_abc", None),
            (b"ops_abc\n\n", None),
            (b"ops\r\nabc", None),
            (b"ops\0abc", None),
            (b"OP_CONNECT_TOKEN=ops_abc", None),
            (b"\xff\xfe", None),
        ] {
            let (_dir, path) = credential_file(contents);
            assert_eq!(
                read_credential_file(Path::new(&path)).as_deref(),
                expected,
                "{contents:?}"
            );
        }
        assert_eq!(
            read_credential_file(Path::new("/nonexistent/vk-cred")),
            None
        );
    }

    #[test]
    fn server_credentials_come_from_named_files_and_never_echo_them() {
        let (_connect_dir, connect) = credential_file(b"connect-secret\n");
        let (_sa_dir, service_account) = credential_file(b"sa-secret");
        let env = |pairs: Vec<(&'static str, String)>| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| value.clone())
            }
        };

        assert!(server_credentials_from(env(vec![])).unwrap().is_empty());

        let both = server_credentials_from(env(vec![
            (SERVER_CONNECT_TOKEN_FILE, connect.clone()),
            (CONNECT_HOST, "http://connect:8080".into()),
            (SERVER_SERVICE_ACCOUNT_TOKEN_FILE, service_account.clone()),
        ]))
        .unwrap();
        assert_eq!(both[CONNECT_HOST], "http://connect:8080");
        assert_eq!(both[CONNECT_TOKEN], "connect-secret");
        assert_eq!(both[TOKEN], "sa-secret");
        // Connect is preferred, the service account is the fallback.
        let selected = select_credentials(&both, None).unwrap();
        assert_eq!(
            selected.connect,
            Some(("http://connect:8080".into(), "connect-secret".into()))
        );
        assert_eq!(selected.service_account.as_deref(), Some("sa-secret"));

        // A Connect token without a host is a configuration error.
        assert!(matches!(
            server_credentials_from(env(vec![(SERVER_CONNECT_TOKEN_FILE, connect)])),
            Err(EnvironmentSecretError::IncompleteConnect)
        ));

        let (_bad_dir, bad) = credential_file(b"  padded-secret  ");
        let error = server_credentials_from(env(vec![(SERVER_SERVICE_ACCOUNT_TOKEN_FILE, bad)]))
            .unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains(SERVER_SERVICE_ACCOUNT_TOKEN_FILE),
            "{message}"
        );
        assert!(!message.contains("padded-secret"), "{message}");
    }

    fn values(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[tokio::test]
    async fn literals_need_no_provider_and_preserve_bytes() {
        let input = values(&[
            ("EMPTY", ""),
            ("TEXT", " prefix op://vault/item/field\n"),
            ("NUMBER", "123"),
        ]);
        assert_eq!(
            resolve_environment_secrets(input.clone()).await.unwrap(),
            input
        );
    }

    const SA: Credential<'static> = Credential::ServiceAccount("token");

    fn breaker() -> ConnectBreaker {
        ConnectBreaker::new(CONNECT_ATTEMPT_TIMEOUT, CONNECT_COOLDOWN)
    }

    fn service_account(token: &str) -> Credentials {
        Credentials {
            connect: None,
            service_account: Some(token.to_owned()),
        }
    }

    fn sa_token(values: &HashMap<String, String>, fallback: Option<&str>) -> Option<String> {
        select_credentials(values, fallback)
            .ok()
            .and_then(|credentials| credentials.service_account)
    }

    const INVALID: [&str; 5] = [
        "",
        "  ",
        "op://vault/item/token",
        "\"op://vault/item/token\"",
        "bad\0token",
    ];

    #[test]
    fn explicit_token_wins_and_invalid_tokens_never_fall_back() {
        let input = values(&[(TOKEN, "configured")]);
        assert_eq!(
            sa_token(&input, Some("host")).as_deref(),
            Some("configured")
        );
        assert_eq!(
            sa_token(&HashMap::new(), Some("host")).as_deref(),
            Some("host")
        );
        assert!(matches!(
            select_credentials(&HashMap::new(), None),
            Err(EnvironmentSecretError::MissingToken)
        ));
        for invalid in INVALID {
            assert!(select_credentials(&values(&[(TOKEN, invalid)]), Some("host")).is_err());
        }
    }

    #[test]
    fn connect_comes_only_from_the_organization_as_a_complete_literal_pair() {
        let connect = values(&[(CONNECT_HOST, "http://connect:8080"), (CONNECT_TOKEN, "ct")]);
        assert_eq!(
            select_credentials(&connect, None).unwrap(),
            Credentials {
                connect: Some(("http://connect:8080".into(), "ct".into())),
                service_account: None,
            }
        );
        // The service-account token stays the fallback, explicit or ambient.
        assert_eq!(
            select_credentials(&connect, Some("host"))
                .unwrap()
                .service_account
                .as_deref(),
            Some("host")
        );
        let mut both = connect.clone();
        both.insert(TOKEN.into(), "org".into());
        assert_eq!(
            select_credentials(&both, Some("host"))
                .unwrap()
                .service_account
                .as_deref(),
            Some("org")
        );
        // An unusable ambient token is ignored when Connect is configured, but
        // an unusable organization token is still an error.
        assert_eq!(
            select_credentials(&connect, Some(" "))
                .unwrap()
                .service_account,
            None
        );
        both.insert(TOKEN.into(), " ".into());
        assert!(matches!(
            select_credentials(&both, Some("host")),
            Err(EnvironmentSecretError::MissingToken)
        ));
        // Half a pair, or an unusable half, never silently falls back.
        for pair in [
            values(&[(CONNECT_HOST, "http://connect:8080")]),
            values(&[(CONNECT_TOKEN, "ct")]),
        ] {
            assert!(matches!(
                select_credentials(&pair, Some("host")),
                Err(EnvironmentSecretError::IncompleteConnect)
            ));
        }
        for invalid in INVALID.into_iter().filter(|value| !value.trim().is_empty()) {
            let pair = values(&[
                (CONNECT_HOST, "http://connect:8080"),
                (CONNECT_TOKEN, invalid),
            ]);
            assert!(matches!(
                select_credentials(&pair, Some("host")),
                Err(EnvironmentSecretError::IncompleteConnect)
            ));
        }
        // Empty values count as not configured.
        let blank = values(&[(CONNECT_HOST, ""), (CONNECT_TOKEN, " ")]);
        assert_eq!(sa_token(&blank, Some("host")).as_deref(), Some("host"));
    }

    #[tokio::test(start_paused = true)]
    async fn public_contract_checks_credentials_cli_and_total_timeout() {
        let input = values(&[("SECRET", "op://vault/item/field")]);
        let unavailable = || std::future::ready(None);
        assert!(matches!(
            resolve_with_provider(input.clone(), None, unavailable()).await,
            Err(EnvironmentSecretError::MissingToken)
        ));
        assert!(matches!(
            resolve_with_provider(input.clone(), Some("token"), unavailable()).await,
            Err(EnvironmentSecretError::MissingCli)
        ));
        assert!(matches!(
            resolve_with_provider(input, Some("token"), std::future::pending()).await,
            Err(EnvironmentSecretError::TimedOut)
        ));
        let literals = values(&[("A", "literal")]);
        assert_eq!(
            resolve_with_provider(literals.clone(), None, std::future::pending())
                .await
                .unwrap(),
            literals
        );
    }

    #[test]
    fn reader_does_not_put_token_in_argv_and_clears_ambient_op_settings() {
        let command = read_command(
            Path::new("op"),
            &Credential::ServiceAccount("test-token"),
            "op://v/i/f",
        );
        let command = command.as_std();
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["read", "--no-newline", "--", "op://v/i/f"]
        );
        let env = command.get_envs().collect::<HashMap<_, _>>();
        assert_eq!(
            env[std::ffi::OsStr::new(TOKEN)],
            Some(std::ffi::OsStr::new("test-token"))
        );
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("OP_") && key != TOKEN {
                assert_eq!(env[key.as_os_str()], None);
            }
        }
        for absent in [CONNECT_HOST, CONNECT_TOKEN] {
            assert!(
                env.get(std::ffi::OsStr::new(absent))
                    .is_none_or(Option::is_none)
            );
        }

        let command = read_command(
            Path::new("op"),
            &Credential::Connect {
                host: "http://connect:8080",
                token: "connect-token",
            },
            "op://v/i/f",
        );
        let env = command.as_std().get_envs().collect::<HashMap<_, _>>();
        assert_eq!(
            env[std::ffi::OsStr::new(CONNECT_HOST)],
            Some(std::ffi::OsStr::new("http://connect:8080"))
        );
        assert_eq!(
            env[std::ffi::OsStr::new(CONNECT_TOKEN)],
            Some(std::ffi::OsStr::new("connect-token"))
        );
        assert!(
            env.get(std::ffi::OsStr::new(TOKEN))
                .is_none_or(Option::is_none)
        );
    }

    #[cfg(unix)]
    fn fake_op(body: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("op");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        (dir, path)
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn mixed_values_use_explicit_token_and_literal_arguments_without_recursion() {
        let (_dir, binary) = fake_op(
            r#"
[ "$1" = read ] && [ "$2" = --no-newline ] && [ "$3" = -- ] || exit 1
[ "$OP_SERVICE_ACCOUNT_TOKEN" = configured ] || exit 1
case "$4" in
  'op://vault/item/field with spaces') printf '  secret\n\n';;
  'op://vault/item/other') printf 'op://returned/literal/value';;
  *) exit 1;;
esac
"#,
        );
        let original = values(&[
            ("A", "op://vault/item/field with spaces"),
            ("B", "op://vault/item/other"),
            ("C", "literal"),
            ("D", "\"op://vault/item/other\""),
            ("E", "\"quoted literal\""),
            (TOKEN, "configured"),
        ]);
        let result = resolve_with_binary(
            original.clone(),
            &service_account("configured"),
            &binary,
            &breaker(),
        )
        .await
        .unwrap();
        assert_eq!(result["A"], "  secret\n\n");
        assert_eq!(result["B"], "op://returned/literal/value");
        assert_eq!(result["C"], "literal");
        assert_eq!(result["D"], "op://returned/literal/value");
        assert_eq!(result["E"], "\"quoted literal\"");
        assert_eq!(result[TOKEN], "configured");
        assert_eq!(original["A"], "op://vault/item/field with spaces");
    }

    /// A fake `op` that records which credential each call used (one line per
    /// call in `<binary>.log`) and runs `connect` or `service_account` for it.
    /// A call that receives both credentials fails the test.
    #[cfg(unix)]
    fn fake_routed_op(
        connect: &str,
        service_account: &str,
    ) -> (tempfile::TempDir, std::path::PathBuf) {
        fake_op(&format!(
            r#"
if [ -n "$OP_CONNECT_HOST" ] && [ -n "$OP_SERVICE_ACCOUNT_TOKEN" ]; then echo both >> "$0.log"; exit 99; fi
if [ -n "$OP_CONNECT_HOST" ]; then
  [ "$OP_CONNECT_HOST" = http://connect:8080 ] && [ "$OP_CONNECT_TOKEN" = ct ] || exit 98
  echo connect >> "$0.log"
  {connect}
else
  [ "$OP_SERVICE_ACCOUNT_TOKEN" = sa ] || exit 97
  echo sa >> "$0.log"
  {service_account}
fi
"#
        ))
    }

    #[cfg(unix)]
    fn calls(binary: &Path) -> Vec<String> {
        std::fs::read_to_string(binary.with_extension("log"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    #[cfg(unix)]
    fn connect_credentials(service_account: Option<&str>) -> Credentials {
        Credentials {
            connect: Some(("http://connect:8080".into(), "ct".into())),
            service_account: service_account.map(str::to_owned),
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn connect_is_tried_first_and_the_service_account_is_the_fallback() {
        let (_dir, binary) = fake_routed_op(
            r#"case "$4" in *cached*) printf from-connect;; *) echo '"x" isn'"'"'t an item' >&2; exit 1;; esac"#,
            "printf from-sa",
        );
        let result = resolve_with_binary(
            values(&[("A", "op://v/cached/f")]),
            &connect_credentials(Some("sa")),
            &binary,
            &breaker(),
        )
        .await
        .unwrap();
        assert_eq!(result["A"], "from-connect");
        assert_eq!(calls(&binary), ["connect"]);

        let result = resolve_with_binary(
            values(&[("B", "op://v/elsewhere/f")]),
            &connect_credentials(Some("sa")),
            &binary,
            &breaker(),
        )
        .await
        .unwrap();
        assert_eq!(result["B"], "from-sa");
        assert_eq!(calls(&binary), ["connect", "connect", "sa"]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn connect_without_a_fallback_reports_its_own_failure() {
        let (_dir, binary) = fake_routed_op(
            "printf 'private-value'; echo '[ERROR] (401) Unauthorized' >&2; exit 1",
            "exit 1",
        );
        let error = resolve_with_binary(
            values(&[("GH_TOKEN", "op://private/ref/field")]),
            &connect_credentials(None),
            &binary,
            &breaker(),
        )
        .await
        .unwrap_err();
        assert!(matches!(
            &error,
            EnvironmentSecretError::ReadFailed {
                variable: Some(name),
                via: Via::Connect,
                cause: LookupFailure::Unauthorized,
            } if name == "GH_TOKEN"
        ));
        assert!(!format!("{error:?} {error}").contains("private-"));
        assert_eq!(calls(&binary), ["connect"]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_silent_connect_is_waited_on_once_per_launch() {
        let (_dir, binary) = fake_routed_op("exec sleep 30", "printf from-sa");
        let started = std::time::Instant::now();
        let result = resolve_with_binary(
            values(&[
                ("A", "op://v/a/f"),
                ("B", "op://v/b/f"),
                ("C", "op://v/c/f"),
            ]),
            &connect_credentials(Some("sa")),
            &binary,
            &ConnectBreaker::new(Duration::from_millis(300), CONNECT_COOLDOWN),
        )
        .await
        .unwrap();
        assert!(
            ["A", "B", "C"]
                .iter()
                .all(|name| result[*name] == "from-sa")
        );
        assert_eq!(calls(&binary), ["connect", "sa", "sa", "sa"]);
        assert!(started.elapsed() < Duration::from_secs(5));

        let (_dir, binary) = fake_routed_op("exec sleep 30", "exit 1");
        let error = resolve_with_binary(
            values(&[("A", "op://v/a/f"), ("B", "op://v/b/f")]),
            &connect_credentials(None),
            &binary,
            &ConnectBreaker::new(Duration::from_millis(300), CONNECT_COOLDOWN),
        )
        .await
        .unwrap_err();
        assert!(matches!(
            error,
            EnvironmentSecretError::ReadFailed {
                via: Via::Connect,
                cause: LookupFailure::Unknown,
                ..
            }
        ));
        assert_eq!(calls(&binary), ["connect"]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_silent_connect_is_skipped_by_later_calls_until_the_cooldown_passes() {
        // Env Vars and each GitHub owner token resolve in separate calls.
        let (_dir, binary) = fake_routed_op("exec sleep 30", "printf from-sa");
        let shared = ConnectBreaker::new(Duration::from_millis(300), CONNECT_COOLDOWN);
        for name in ["GH_TOKEN", "VALUE", "VALUE"] {
            let result = resolve_with_binary(
                values(&[(name, "op://v/a/f")]),
                &connect_credentials(Some("sa")),
                &binary,
                &shared,
            )
            .await
            .unwrap();
            assert_eq!(result[name], "from-sa");
        }
        assert_eq!(calls(&binary), ["connect", "sa", "sa", "sa"]);

        let (_dir, binary) = fake_routed_op("exec sleep 30", "printf from-sa");
        let expiring = ConnectBreaker::new(Duration::from_millis(300), Duration::ZERO);
        for _ in 0..2 {
            resolve_with_binary(
                values(&[("A", "op://v/a/f")]),
                &connect_credentials(Some("sa")),
                &binary,
                &expiring,
            )
            .await
            .unwrap();
        }
        assert_eq!(calls(&binary), ["connect", "sa", "connect", "sa"]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn an_unusable_field_from_connect_is_not_fetched_again() {
        let (_dir, binary) = fake_routed_op("printf '\\000'", "printf from-sa");
        assert!(matches!(
            resolve_with_binary(
                values(&[("A", "op://v/a/f")]),
                &connect_credentials(Some("sa")),
                &binary,
                &breaker(),
            )
            .await,
            Err(EnvironmentSecretError::InvalidValue)
        ));
        assert_eq!(calls(&binary), ["connect"]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn failures_never_return_partial_maps_or_provider_output() {
        let (_dir, binary) =
            fake_op("printf 'private-value'; printf 'private-diagnostic' >&2; exit 1");
        let result = resolve_with_binary(
            values(&[("A", "op://private/path/field"), ("B", "literal")]),
            &service_account("private-token"),
            &binary,
            &breaker(),
        )
        .await;
        let error = result.unwrap_err();
        assert!(matches!(
            &error,
            EnvironmentSecretError::ReadFailed {
                variable: Some(name),
                via: Via::ServiceAccount,
                cause: LookupFailure::Unknown,
            } if name == "A"
        ));
        assert!(!format!("{error:?} {error}").contains("private-"));
        assert!(matches!(
            read_reference(Path::new("/nonexistent/vk-op"), &SA, "op://v/i/f").await,
            Err(EnvironmentSecretError::ReadFailed {
                variable: None,
                via: Via::ServiceAccount,
                cause: LookupFailure::Unknown,
            })
        ));
    }

    #[test]
    fn classifies_op_diagnostics_without_keeping_them() {
        // Messages as printed by op 2.34, plus the throttling forms 1Password
        // documents for service accounts.
        for (stderr, expected) in [
            (
                "[ERROR] could not read secret 'op://Homelab/x/credential': could not get item Homelab/x: \"x\" isn't an item in the \"Homelab\" vault.",
                LookupFailure::NotFound,
            ),
            (
                "[ERROR] could not get item V/x: \"V\" isn't a vault in this account.",
                LookupFailure::NotFound,
            ),
            (
                "[ERROR] item 'Homelab/x' does not have a field 'nosuchfield'",
                LookupFailure::NotFound,
            ),
            (
                "[ERROR] error initializing client: Validation: (failed to session.DecodeSACredentials)",
                LookupFailure::Unauthorized,
            ),
            (
                "[ERROR] (401) Unauthorized: You aren't authorized",
                LookupFailure::Unauthorized,
            ),
            (
                "[ERROR] (429) Too Many Requests: rate limit exceeded",
                LookupFailure::RateLimited,
            ),
            (
                "[ERROR] Too many requests. Your client has been rate-limited.",
                LookupFailure::RateLimited,
            ),
            (
                "[ERROR] cannot create directory \"/x/.config/op\": permission denied",
                LookupFailure::Unknown,
            ),
            ("", LookupFailure::Unknown),
            // Names echoed by op never decide the cause.
            (
                "[ERROR] could not read secret 'op://Homelab/token429/credential': item 'Homelab/token429' does not have a field 'credential'",
                LookupFailure::NotFound,
            ),
            (
                "[ERROR] could not get item Homelab/rate limit 401: \"rate limit 401\" isn't an item in the \"Homelab\" vault.",
                LookupFailure::NotFound,
            ),
            (
                "[ERROR] could not read secret 'op://Unauthorized/x/y': exit status 1",
                LookupFailure::Unknown,
            ),
            (
                "[ERROR] could not read secret 'op://Homelab/David's (429) PAT/x': item 'Homelab/David's (429) PAT' does not have a field 'x'",
                LookupFailure::NotFound,
            ),
            (
                "[ERROR] (401) You aren't authorized to read 'Homelab/rate limit'",
                LookupFailure::Unauthorized,
            ),
        ] {
            assert_eq!(classify(stderr.as_bytes()), expected, "{stderr}");
        }
    }

    #[test]
    fn messages_name_the_variable_only_when_given_and_keep_the_cause() {
        let named = EnvironmentSecretError::ReadFailed {
            variable: Some("GH_TOKEN".into()),
            via: Via::ServiceAccount,
            cause: LookupFailure::RateLimited,
        };
        let message = named.to_string();
        assert!(message.starts_with(
            "1Password lookup for GH_TOKEN failed through the service account (OP_SERVICE_ACCOUNT_TOKEN): "
        ));
        assert!(message.contains("rate-limited"));
        let unnamed = named.without_variable().to_string();
        assert!(unnamed.starts_with("1Password lookup failed through the service account"));
        assert!(unnamed.contains("rate-limited"));
        assert_eq!(
            EnvironmentSecretError::read_failed(Via::Connect, LookupFailure::Unknown).to_string(),
            "1Password lookup failed through 1Password Connect (OP_CONNECT_HOST): check the reference, token, vault permissions, and connectivity"
        );
        assert!(matches!(
            EnvironmentSecretError::TimedOut.without_variable(),
            EnvironmentSecretError::TimedOut
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn rate_limited_reader_names_the_variable_and_hides_provider_output() {
        let (_dir, binary) = fake_op(
            "printf 'private-value'; printf '[ERROR] (429) Too Many Requests for op://private/ref\\n' >&2; exit 1",
        );
        let error = resolve_with_binary(
            values(&[("GH_TOKEN", "op://private/ref/field"), ("B", "literal")]),
            &service_account("private-token"),
            &binary,
            &breaker(),
        )
        .await
        .unwrap_err();
        assert!(matches!(
            &error,
            EnvironmentSecretError::ReadFailed {
                variable: Some(name),
                via: Via::ServiceAccount,
                cause: LookupFailure::RateLimited,
            } if name == "GH_TOKEN"
        ));
        let rendered = format!("{error:?} {error}");
        for private in ["private-", "op://", "Too Many", "429"] {
            assert!(!rendered.contains(private), "{private} leaked: {rendered}");
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn verbose_diagnostics_cannot_stall_the_reader() {
        // Far past the stderr bound and the pipe buffer, then a failing exit.
        let (_dir, binary) = fake_op(
            "i=0; while [ $i -lt 20000 ]; do echo 'rate limit noise' >&2; i=$((i+1)); done; exit 1",
        );
        let result = tokio::time::timeout(
            Duration::from_secs(10),
            read_reference(&binary, &SA, "op://v/i/f"),
        )
        .await
        .expect("reader must not block on a full stderr pipe");
        assert!(matches!(
            result,
            Err(EnvironmentSecretError::ReadFailed { .. })
        ));
        let (_dir, binary) = fake_op("exec yes private-value");
        let result = tokio::time::timeout(
            Duration::from_secs(10),
            read_reference(&binary, &SA, "op://v/i/f"),
        )
        .await
        .expect("reader must not block on endless output");
        assert!(matches!(result, Err(EnvironmentSecretError::InvalidValue)));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn rejects_non_environment_values_and_accepts_empty_fields() {
        for body in [
            "printf '\\000'",
            "printf '\\377'",
            "i=0; while [ $i -lt 65537 ]; do printf x; i=$((i+1)); done",
        ] {
            let (_dir, binary) = fake_op(body);
            assert!(matches!(
                read_reference(&binary, &SA, "op://v/i/f").await,
                Err(EnvironmentSecretError::InvalidValue)
            ));
        }
        let (_dir, binary) = fake_op("exit 0");
        assert_eq!(
            read_reference(&binary, &SA, "op://v/i/f").await.unwrap(),
            ""
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancellation_kills_the_reader() {
        let (_dir, binary) = fake_op("printf '%s' \"$$\" > \"$0.pid\"; exec sleep 60");
        let result = tokio::time::timeout(
            Duration::from_millis(300),
            read_reference(&binary, &SA, "op://v/i/f"),
        )
        .await;
        assert!(result.is_err());
        let pid = std::fs::read_to_string(binary.with_extension("pid")).unwrap();
        let mut exited = false;
        for _ in 0..50 {
            let status = Command::new("kill")
                .args(["-0", pid.trim()])
                .stderr(Stdio::null())
                .status()
                .await
                .unwrap();
            if !status.success() {
                exited = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(
            exited,
            "cancelled secret reader must not survive its request"
        );
    }
}
