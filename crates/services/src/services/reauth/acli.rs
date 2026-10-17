//! Atlassian CLI (`acli confluence`): log in from an API token in 1Password.
//!
//! No browser. The token is resolved through 1Password Connect and written to
//! acli's stdin — with the trailing newline acli requires, or it fails with
//! "failed to read token from standard input". It never reaches argv or logs.
//! Proof is a real page read (the catalog probe), never `acli … auth status`,
//! which on the Jira side reports success while every command still 401s.
//!
//! Scope: acli keeps its login under this service's home, so this repairs it
//! for agents on the coordinator; cluster workers keep their own state.

use std::process::Stdio;

use tokio::io::AsyncWriteExt;

use super::ReauthError;
use crate::services::{
    cli_tools::{self, CliToolId},
    entra_mint::{self, OnePasswordConnect, OpRef, Progress},
};

#[derive(Debug, Clone)]
pub struct ApiTokenConfig {
    pub site: String,
    pub email: OpRef,
    pub token: OpRef,
    pub connect: OnePasswordConnect,
}

fn required(key: &str) -> Result<String, ReauthError> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .ok_or_else(|| ReauthError::NotConfigured(format!("{key} is not set")))
}

/// An Atlassian site host: letters, digits, dots and hyphens only.
fn valid_site(site: &str) -> bool {
    !site.is_empty()
        && site.len() <= 253
        && site
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
}

impl ApiTokenConfig {
    pub fn from_env() -> Result<Self, ReauthError> {
        let site = required("VK_ATLASSIAN_SITE")?;
        if !valid_site(&site) {
            return Err(ReauthError::NotConfigured(
                "VK_ATLASSIAN_SITE must be a bare host such as example.atlassian.net".into(),
            ));
        }
        let parse = |key: &str| -> Result<OpRef, ReauthError> {
            entra_mint::parse_op_ref(&required(key)?)
                .map_err(|e| ReauthError::NotConfigured(format!("{key}: {e}")))
        };
        let email = parse("VK_ATLASSIAN_EMAIL_REF")?;
        let token = parse("VK_ATLASSIAN_TOKEN_REF")?;
        // Verification needs a page; without one success could not be proven.
        required("VK_ATLASSIAN_VERIFY_PAGE_ID")?;
        let connect = OnePasswordConnect::from_env()
            .map_err(|e| ReauthError::NotConfigured(e.to_string()))?;
        Ok(Self {
            site,
            email,
            token,
            connect,
        })
    }
}

/// Remove every occurrence of `secret` before output reaches a transcript.
pub(crate) fn scrub(output: &str, secret: &str) -> String {
    if secret.is_empty() {
        return output.to_string();
    }
    output.replace(secret, "[redacted]")
}

pub async fn run(progress: &Progress) -> Result<(), ReauthError> {
    let cfg = ApiTokenConfig::from_env()?;
    let executable = cli_tools::effective_binary_for(CliToolId::Acli)
        .await
        .ok_or_else(|| {
            ReauthError::NotConfigured(
                "the Atlassian CLI is not installed; install it from CLI Tools".into(),
            )
        })?;
    progress.say(format!(
        "Resolving the Atlassian account and API token ({}, {}) through 1Password Connect…",
        cfg.email, cfg.token
    ));
    let http = entra_mint::http_client()?;
    let email = entra_mint::resolve_op_ref(&http, &cfg.connect, &cfg.email).await?;
    let token = entra_mint::resolve_op_ref(&http, &cfg.connect, &cfg.token).await?;

    progress.say(format!("Logging acli confluence in to {}…", cfg.site));
    let mut child = tokio::process::Command::new(&executable)
        .args([
            "confluence",
            "auth",
            "login",
            "--site",
            &cfg.site,
            "--email",
            &email,
            "--token",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| ReauthError::Failed(format!("could not start acli: {e}")))?;
    {
        let mut stdin = child.stdin.take().expect("piped");
        stdin
            .write_all(format!("{token}\n").as_bytes())
            .await
            .map_err(|e| ReauthError::Failed(format!("could not pass the token to acli: {e}")))?;
        stdin.shutdown().await.ok();
    }
    let output = tokio::time::timeout(std::time::Duration::from_secs(60), child.wait_with_output())
        .await
        .map_err(|_| ReauthError::Failed("acli login timed out".into()))?
        .map_err(|e| ReauthError::Failed(format!("acli login failed: {e}")))?;
    let text = scrub(
        &format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
        &token,
    );
    for line in text.lines().filter(|l| !l.trim().is_empty()).take(20) {
        progress.say(format!("  {line}"));
    }
    if !output.status.success() {
        return Err(ReauthError::Failed(format!(
            "acli confluence auth login failed: {}",
            text.trim()
        )));
    }
    progress.say("acli accepted the token; verifying with a page read…");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrub_removes_every_copy_of_the_token() {
        let out = scrub("error: token ATATT3x rejected (ATATT3x)", "ATATT3x");
        assert_eq!(out, "error: token [redacted] rejected ([redacted])");
        assert_eq!(scrub("fine", ""), "fine");
    }

    #[test]
    fn site_must_be_a_bare_host() {
        assert!(valid_site("sweetgreen.atlassian.net"));
        assert!(!valid_site("https://sweetgreen.atlassian.net"));
        assert!(!valid_site("a b"));
        assert!(!valid_site(""));
    }

    #[test]
    fn missing_configuration_names_the_variable() {
        // Reads the environment only; skipped where a developer has the
        // variable set for a live run.
        if std::env::var_os("VK_ATLASSIAN_SITE").is_some() {
            return;
        }
        let err = ApiTokenConfig::from_env().unwrap_err().to_string();
        assert!(err.contains("VK_ATLASSIAN_SITE"), "{err}");
    }
}
