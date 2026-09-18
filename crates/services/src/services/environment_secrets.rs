//! Resolve stored 1Password references only while preparing a child environment.
//! Never persist resolved values or include provider output in diagnostics.

use std::{collections::HashMap, path::Path, process::Stdio, time::Duration};

use thiserror::Error;
use tokio::{io::AsyncReadExt, process::Command};

use super::cli_tools::{CliToolId, effective_binary_for};

const TOKEN: &str = "OP_SERVICE_ACCOUNT_TOKEN";
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_SECRET_BYTES: u64 = 64 * 1024;

#[derive(Debug, Error)]
pub enum EnvironmentSecretError {
    #[error(
        "1Password references require a nonempty literal OP_SERVICE_ACCOUNT_TOKEN in organization Env Vars or the Vibe Kanban service environment"
    )]
    MissingToken,
    #[error(
        "1Password CLI is unavailable; install it in CLI Tools or on the Vibe Kanban service PATH"
    )]
    MissingCli,
    #[error(
        "1Password lookup failed; check the reference, service-account token, and vault permissions"
    )]
    ReadFailed,
    #[error("1Password lookup timed out; check service connectivity and retry")]
    TimedOut,
    #[error("1Password field must be NUL-free UTF-8 text of at most 64 KiB")]
    InvalidValue,
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
    let token = select_token(&values, fallback)?.to_owned();
    tokio::time::timeout(RESOLVE_TIMEOUT, async {
        let binary = binary.await.ok_or(EnvironmentSecretError::MissingCli)?;
        resolve_with_binary(values, &token, &binary).await
    })
    .await
    .map_err(|_| EnvironmentSecretError::TimedOut)?
}

fn has_references(values: &HashMap<String, String>) -> bool {
    values.values().any(|value| value.starts_with("op://"))
}

fn select_token<'a>(
    values: &'a HashMap<String, String>,
    fallback: Option<&'a str>,
) -> Result<&'a str, EnvironmentSecretError> {
    values
        .get(TOKEN)
        .map(String::as_str)
        .or(fallback)
        .filter(|token| {
            !token.trim().is_empty() && !token.starts_with("op://") && !token.contains('\0')
        })
        .ok_or(EnvironmentSecretError::MissingToken)
}

async fn resolve_with_binary(
    mut values: HashMap<String, String>,
    token: &str,
    binary: &Path,
) -> Result<HashMap<String, String>, EnvironmentSecretError> {
    for value in values.values_mut() {
        if value.starts_with("op://") {
            *value = read_reference(binary, token, value).await?;
        }
    }
    Ok(values)
}

fn read_command(binary: &Path, token: &str, reference: &str) -> Command {
    let mut command = Command::new(binary);
    // Ambient Connect credentials, account selectors, interactive sessions, and
    // OP_DEBUG must not change the selected service-account identity or logging.
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("OP_") {
            command.env_remove(key);
        }
    }
    command
        .args(["read", "--no-newline", "--", reference])
        .env(TOKEN, token)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    command
}

async fn read_reference(
    binary: &Path,
    token: &str,
    reference: &str,
) -> Result<String, EnvironmentSecretError> {
    let mut child = read_command(binary, token, reference)
        .spawn()
        .map_err(|_| EnvironmentSecretError::ReadFailed)?;
    let mut bytes = Vec::new();
    child
        .stdout
        .take()
        .ok_or(EnvironmentSecretError::ReadFailed)?
        .take(MAX_SECRET_BYTES + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| EnvironmentSecretError::ReadFailed)?;
    if bytes.len() as u64 > MAX_SECRET_BYTES {
        return Err(EnvironmentSecretError::InvalidValue);
    }
    if !child
        .wait()
        .await
        .map_err(|_| EnvironmentSecretError::ReadFailed)?
        .success()
    {
        return Err(EnvironmentSecretError::ReadFailed);
    }
    if bytes.contains(&0) {
        return Err(EnvironmentSecretError::InvalidValue);
    }
    String::from_utf8(bytes).map_err(|_| EnvironmentSecretError::InvalidValue)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn explicit_token_wins_and_invalid_tokens_never_fall_back() {
        let input = values(&[(TOKEN, "configured")]);
        assert_eq!(select_token(&input, Some("host")).unwrap(), "configured");
        assert_eq!(select_token(&HashMap::new(), Some("host")).unwrap(), "host");
        assert!(select_token(&HashMap::new(), None).is_err());
        for invalid in ["", "  ", "op://vault/item/token", "bad\0token"] {
            assert!(select_token(&values(&[(TOKEN, invalid)]), Some("host")).is_err());
        }
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
        let command = read_command(Path::new("op"), "test-token", "op://v/i/f");
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
            (TOKEN, "configured"),
        ]);
        let result = resolve_with_binary(original.clone(), "configured", &binary)
            .await
            .unwrap();
        assert_eq!(result["A"], "  secret\n\n");
        assert_eq!(result["B"], "op://returned/literal/value");
        assert_eq!(result["C"], "literal");
        assert_eq!(result[TOKEN], "configured");
        assert_eq!(original["A"], "op://vault/item/field with spaces");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn failures_never_return_partial_maps_or_provider_output() {
        let (_dir, binary) =
            fake_op("printf 'private-value'; printf 'private-diagnostic' >&2; exit 1");
        let result = resolve_with_binary(
            values(&[("A", "op://private/path/field"), ("B", "literal")]),
            "private-token",
            &binary,
        )
        .await;
        let error = result.unwrap_err();
        assert!(matches!(error, EnvironmentSecretError::ReadFailed));
        assert!(!format!("{error:?} {error}").contains("private-"));
        assert!(matches!(
            read_reference(Path::new("/nonexistent/vk-op"), "token", "op://v/i/f").await,
            Err(EnvironmentSecretError::ReadFailed)
        ));
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
                read_reference(&binary, "token", "op://v/i/f").await,
                Err(EnvironmentSecretError::InvalidValue)
            ));
        }
        let (_dir, binary) = fake_op("exit 0");
        assert_eq!(
            read_reference(&binary, "token", "op://v/i/f")
                .await
                .unwrap(),
            ""
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancellation_kills_the_reader() {
        let (_dir, binary) = fake_op("printf '%s' \"$$\" > \"$0.pid\"; exec sleep 60");
        let result = tokio::time::timeout(
            Duration::from_millis(300),
            read_reference(&binary, "token", "op://v/i/f"),
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
