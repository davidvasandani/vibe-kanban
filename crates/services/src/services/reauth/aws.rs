//! AWS SSO: complete `aws sso login`'s device authorization with no human.
//!
//! The same command Settings runs in a PTY (`--use-device-code`), plus
//! `--no-browser`, with output piped. The printed verification URL is opened
//! in the firecrawl browser on the persistent Entra profile: the SAML hop to
//! Entra is usually silent there, and [`entra_mint::drive`] completes it from
//! 1Password when it is not. The AWS pages that follow (*Confirm and
//! continue*, *Allow access*) are clicked, and the CLI — which has been
//! polling all along — exits once the grant is approved and writes its own
//! `~/.aws/sso/cache`. The device grant is AWS's, not Microsoft's, so the
//! tenant's device-code block does not apply to it.

use std::{
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};

use regex::Regex;
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;

use super::{ReauthError, ReauthTargetId};
use crate::services::{
    aws_sso,
    entra_mint::{self, BrowserSession, EntraConfig, EntraError, PageVerdict, Probe, Progress},
};

/// How long the CLI may take to print its prompt.
const PROMPT_TIMEOUT: Duration = Duration::from_secs(30);
/// Browser budget per attempt: the device grant itself lasts ~10 minutes.
const DRIVE_TIMEOUT: Duration = Duration::from_secs(240);
/// After the approval page, how long the CLI's poll may take to notice.
const EXIT_TIMEOUT: Duration = Duration::from_secs(45);

fn user_code_re() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b[A-Z0-9]{4}-[A-Z0-9]{4}\b").expect("valid regex"))
}

/// The URL to open from `aws sso login --no-browser` output: the pre-filled
/// one when printed, else the bare verification URL plus the code.
pub(crate) fn parse_device_prompt(output: &str) -> Option<String> {
    let urls: Vec<&str> = output
        .split_whitespace()
        .filter(|w| w.starts_with("https://"))
        .collect();
    if let Some(url) = urls.iter().find(|u| u.contains("user_code=")) {
        return Some((*url).to_string());
    }
    let url = urls.first()?;
    let code = user_code_re().find(output)?.as_str();
    let sep = if url.contains('?') { '&' } else { '?' };
    Some(format!("{url}{sep}user_code={code}"))
}

/// Output with the device code and URL queries removed, safe to log.
fn scrub(output: &str) -> String {
    let no_codes = user_code_re().replace_all(output, "****-****");
    no_codes
        .split_whitespace()
        .map(|w| {
            if w.starts_with("http") {
                entra_mint::redact_url(w)
            } else {
                w.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// AWS hosts the device approval can render on: the legacy SSO portal
/// (`*.awsapps.com`), the device endpoint (`device.sso.<region>.amazonaws.com`)
/// and the newer regional sign-in pages (`<region>.signin.aws`).
fn is_aws_host(host: &str) -> bool {
    host.ends_with(".amazonaws.com")
        || host.ends_with(".awsapps.com")
        || host == "signin.aws"
        || host.ends_with(".signin.aws")
}

/// Clicks the first visible AWS approval button, in the order AWS shows them.
const APPROVE_JS: &str = r#"
for (const name of [/confirm and continue/i, /allow access/i, /^\s*allow\s*$/i]) {
  const b = page.getByRole('button', { name });
  if (await b.count() && await b.first().isVisible().catch(() => false)) {
    await b.first().click();
    return name.source;
  }
}
return null;
"#;

/// What to do on an AWS page (Entra pages are left to the driver).
pub(crate) fn aws_page_verdict(p: &Probe) -> PageVerdict<()> {
    if !is_aws_host(&p.host()) {
        return PageVerdict::Continue;
    }
    if p.txt.contains("request approved") || p.txt.contains("you can close this window") {
        return PageVerdict::Done(());
    }
    if p.txt.contains("request has expired")
        || p.txt.contains("code has expired")
        || p.txt.contains("expired or is invalid")
    {
        return PageVerdict::Fail("AWS reported the device authorization expired".to_string());
    }
    if p.txt.contains("confirm and continue") || p.txt.contains("allow") {
        return PageVerdict::Run(APPROVE_JS.to_string());
    }
    PageVerdict::Continue
}

/// The login child, watched by a task that owns it (so it can be killed on
/// retry and is always reaped).
struct LoginChild {
    output: Arc<Mutex<String>>,
    exit: Arc<Mutex<Option<bool>>>,
    cancel: CancellationToken,
    watcher: tokio::task::JoinHandle<()>,
}

impl LoginChild {
    fn spawn(command: &aws_sso::AwsLoginCommand) -> Result<Self, ReauthError> {
        let mut child = tokio::process::Command::new(&command.executable)
            .args(&command.args)
            .arg("--no-browser")
            .envs(&command.env)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| ReauthError::Failed(format!("could not start aws sso login: {e}")))?;
        let output = Arc::new(Mutex::new(String::new()));
        for mut pipe in [
            child
                .stdout
                .take()
                .map(|p| Box::new(p) as Box<dyn tokio::io::AsyncRead + Send + Unpin>),
            child
                .stderr
                .take()
                .map(|p| Box::new(p) as Box<dyn tokio::io::AsyncRead + Send + Unpin>),
        ]
        .into_iter()
        .flatten()
        {
            let output = output.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                while let Ok(n) = pipe.read(&mut buf).await {
                    if n == 0 {
                        break;
                    }
                    let mut out = output.lock().unwrap_or_else(|e| e.into_inner());
                    if out.len() < 64 * 1024 {
                        out.push_str(&String::from_utf8_lossy(&buf[..n]));
                    }
                }
            });
        }
        let exit = Arc::new(Mutex::new(None));
        let cancel = CancellationToken::new();
        let watcher = {
            let exit = exit.clone();
            let cancel = cancel.clone();
            tokio::spawn(async move {
                tokio::select! {
                    status = child.wait() => {
                        *exit.lock().unwrap_or_else(|e| e.into_inner()) =
                            Some(status.map(|s| s.success()).unwrap_or(false));
                    }
                    _ = cancel.cancelled() => {
                        let _ = child.kill().await;
                    }
                }
            })
        };
        Ok(Self {
            output,
            exit,
            cancel,
            watcher,
        })
    }

    fn output(&self) -> String {
        self.output
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn exited(&self) -> Option<bool> {
        *self.exit.lock().unwrap_or_else(|e| e.into_inner())
    }

    async fn wait_exit(&self, timeout: Duration) -> Option<bool> {
        let deadline = tokio::time::Instant::now() + timeout;
        while tokio::time::Instant::now() < deadline {
            if let Some(ok) = self.exited() {
                return Some(ok);
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
        self.exited()
    }

    async fn stop(self) {
        self.cancel.cancel();
        let _ = self.watcher.await;
    }

    fn failure(&self) -> ReauthError {
        ReauthError::Failed(format!(
            "aws sso login failed: {}",
            tail(&scrub(&self.output()), 400)
        ))
    }
}

fn tail(s: &str, n: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    chars[chars.len().saturating_sub(n)..].iter().collect()
}

async fn login_command(id: &ReauthTargetId) -> Result<aws_sso::AwsLoginCommand, ReauthError> {
    match id {
        ReauthTargetId::AwsSession(name) => Ok(aws_sso::login_command_for_session(name).await?),
        ReauthTargetId::AwsProfile(name) => Ok(aws_sso::login_command_for_profile(name).await?),
        other => Err(ReauthError::InvalidTarget(other.to_string())),
    }
}

/// One attempt: fresh device code, fresh browser session.
async fn attempt(
    cfg: &EntraConfig,
    command: &aws_sso::AwsLoginCommand,
    progress: &Progress,
) -> Result<(), ReauthError> {
    let child = LoginChild::spawn(command)?;
    let result = async {
        let deadline = tokio::time::Instant::now() + PROMPT_TIMEOUT;
        let url = loop {
            if let Some(url) = parse_device_prompt(&child.output()) {
                break url;
            }
            match child.exited() {
                Some(true) => {
                    progress.say("aws sso login completed without a browser (token still valid).");
                    return Ok(());
                }
                Some(false) => return Err(child.failure()),
                None => {}
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(ReauthError::Failed(
                    "aws sso login never printed a verification URL".into(),
                ));
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
        };
        let session = BrowserSession::open_default(cfg, true).await?;
        progress.say(format!(
            "Opened the AWS device page {} on profile '{}'.",
            entra_mint::redact_url(&url),
            cfg.browser_profile
        ));
        let driven = async {
            session.navigate(&url).await?;
            entra_mint::drive(cfg, &session, progress, DRIVE_TIMEOUT, |p| {
                // The CLI finishing is the strongest signal of all.
                match child.exited() {
                    Some(true) => return PageVerdict::Done(()),
                    Some(false) => return PageVerdict::Fail("aws sso login exited".into()),
                    None => {}
                }
                aws_page_verdict(p)
            })
            .await
        }
        .await;
        session.close().await;
        driven?;
        progress.say("AWS approved the request; waiting for the CLI to store its token…");
        match child.wait_exit(EXIT_TIMEOUT).await {
            Some(true) => Ok(()),
            Some(false) => Err(child.failure()),
            None => Err(ReauthError::Failed(
                "aws sso login did not finish after the approval".into(),
            )),
        }
    }
    .await;
    child.stop().await;
    result
}

pub async fn run(id: &ReauthTargetId, progress: &Progress) -> Result<(), ReauthError> {
    let cfg = EntraConfig::from_env().map_err(|e| ReauthError::NotConfigured(e.to_string()))?;
    let command = login_command(id).await?;
    let _guard = aws_sso::try_begin_profile_login(&command.lock_key).ok_or_else(|| {
        ReauthError::Busy("an interactive AWS sign-in for this scope is in progress".into())
    })?;
    let mut n = 1;
    loop {
        progress.say(format!("AWS SSO sign-in, attempt {n}"));
        match attempt(&cfg, &command, progress).await {
            Err(ReauthError::Entra(e)) if entra_mint::should_retry(&e, n) => {
                progress.say(format!("{e}; retrying with a fresh device code"));
                n += 1;
            }
            Err(ReauthError::Entra(EntraError::Config(m))) => {
                return Err(ReauthError::NotConfigured(m));
            }
            other => return other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const V2_OUTPUT: &str = "Browser will not be automatically opened.\n\
        Please visit the following URL:\n\nhttps://device.sso.us-east-1.amazonaws.com/\n\n\
        Then enter the code:\n\nWXYZ-1234\n\n\
        Alternatively, you may visit the following URL which will autofill the code upon loading:\n\
        https://device.sso.us-east-1.amazonaws.com/?user_code=WXYZ-1234\n";

    #[test]
    fn prefers_the_prefilled_url() {
        assert_eq!(
            parse_device_prompt(V2_OUTPUT).as_deref(),
            Some("https://device.sso.us-east-1.amazonaws.com/?user_code=WXYZ-1234")
        );
    }

    #[test]
    fn joins_url_and_code_when_only_both_are_printed() {
        let out = "Please visit https://sweetgreen.awsapps.com/start/#/device\nand enter ABCD-EFGH";
        assert_eq!(
            parse_device_prompt(out).as_deref(),
            Some("https://sweetgreen.awsapps.com/start/#/device?user_code=ABCD-EFGH")
        );
        assert_eq!(parse_device_prompt("starting…"), None);
        assert_eq!(parse_device_prompt("https://x.example/ only"), None);
    }

    #[test]
    fn scrubbed_output_has_no_code_or_query() {
        let s = scrub(V2_OUTPUT);
        assert!(!s.contains("WXYZ-1234"), "{s}");
        assert!(!s.contains("user_code"), "{s}");
        assert!(s.contains("https://device.sso.us-east-1.amazonaws.com/"));
    }

    fn page(url: &str, txt: &str) -> Probe {
        Probe {
            url: url.into(),
            txt: txt.into(),
            ..Default::default()
        }
    }

    #[test]
    fn approval_pages_are_clicked_and_completion_is_detected() {
        let device = "https://device.sso.us-east-1.amazonaws.com/";
        assert!(matches!(
            aws_page_verdict(&page(
                device,
                "authorization requested confirm and continue"
            )),
            PageVerdict::Run(_)
        ));
        assert!(matches!(
            aws_page_verdict(&page(
                "https://sweetgreen.awsapps.com/start/#/device",
                "allow botocore-client-x to access your data? allow access"
            )),
            PageVerdict::Run(_)
        ));
        assert!(matches!(
            aws_page_verdict(&page(device, "request approved you can close this window")),
            PageVerdict::Done(())
        ));
        assert!(matches!(
            aws_page_verdict(&page(device, "your request has expired")),
            PageVerdict::Fail(_)
        ));
    }

    #[test]
    fn regional_signin_hosts_are_aws_pages() {
        assert!(matches!(
            aws_page_verdict(&page(
                "https://us-east-1.signin.aws/platform/login",
                "authorization requested confirm and continue"
            )),
            PageVerdict::Run(_)
        ));
        assert!(matches!(
            aws_page_verdict(&page("https://evil.signin.aws.example/", "allow")),
            PageVerdict::Continue
        ));
    }

    #[test]
    fn non_aws_pages_are_left_to_the_entra_driver() {
        // Entra's "allow" copy must never trigger an AWS click.
        assert!(matches!(
            aws_page_verdict(&page(
                "https://login.microsoftonline.com/t/saml2",
                "allow access? stay signed in"
            )),
            PageVerdict::Continue
        ));
        assert!(matches!(
            aws_page_verdict(&page("https://evil-amazonaws.com.example/", "allow")),
            PageVerdict::Continue
        ));
    }
}
