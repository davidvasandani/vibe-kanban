//! Entra (Microsoft Entra ID) delegated-token minting for app-managed CLI tools.
//!
//! # Why this exists
//!
//! The obvious way to authenticate a CLI tool on a headless host is the
//! device-code flow (`az login --use-device-code`). In tenants that restrict
//! authentication flows by Conditional Access that flow is refused outright —
//! `AADSTS53003`, returned even when the browser completing it runs on a fully
//! Compliant, managed device, because the block is on the *flow*, not the
//! device. The authorization-code flow is not restricted.
//!
//! So this module drives an authorization-code + PKCE flow through the
//! self-hosted firecrawl browser service. Two consequences shape the design:
//!
//! * **No loopback listener.** The browser runs in a sandbox that cannot reach
//!   this host's private address, so the usual `http://localhost:<port>`
//!   redirect is unusable. We redirect to the `nativeclient` endpoint instead
//!   and read the authorization code straight off the browser's URL.
//! * **A persistent browser profile.** The first mint signs in interactively
//!   (password + TOTP from 1Password). The session cookie is saved to the
//!   profile, so every later mint completes with `prompt=none` and no
//!   credentials at all.

use std::{collections::HashMap, path::PathBuf, time::Duration};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64URL};
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::sync::mpsc::UnboundedSender;

/// Redirect target for the auth-code flow. Registered for the Microsoft
/// first-party public clients we mint for, and — critically — a URL the
/// browser can actually load, unlike a loopback port on this host.
const NATIVE_CLIENT_REDIRECT: &str = "https://login.microsoftonline.com/common/oauth2/nativeclient";

/// How long to let an interactive sign-in run before giving up.
const INTERACTIVE_TIMEOUT: Duration = Duration::from_secs(240);
/// Poll interval between browser step probes.
const STEP_INTERVAL: Duration = Duration::from_secs(2);
/// Browser session lifetime; generous enough for an interactive sign-in.
const SESSION_TTL_SECS: u32 = 600;

#[derive(Debug, thiserror::Error)]
pub enum EntraError {
    #[error("Entra minting is not configured: {0}")]
    Config(String),
    #[error("browser service error: {0}")]
    Browser(String),
    #[error("1Password Connect error: {0}")]
    OnePassword(String),
    #[error("Entra returned {0}")]
    Auth(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

/// Where the Entra password and TOTP live in 1Password Connect.
///
/// Connect computes the TOTP server-side and returns it on the field, so we
/// never handle the shared secret or implement RFC 6238 here.
#[derive(Clone, Debug)]
pub struct OnePasswordRef {
    pub host: String,
    pub token: String,
    pub vault: String,
    pub item: String,
    pub password_field: String,
    pub totp_field: String,
}

#[derive(Clone, Debug)]
pub struct EntraConfig {
    pub cdp_url: String,
    pub cdp_token: String,
    pub tenant_id: String,
    pub email: String,
    pub browser_profile: String,
    pub op: OnePasswordRef,
}

fn env_var(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

/// Read a secret given either inline or as a file path, preferring the file.
/// systemd `LoadCredential=` drops credentials in `$CREDENTIALS_DIRECTORY`,
/// which is how the deployed service receives its 1Password Connect token.
fn env_secret(inline: &str, file: &str, credential: Option<&str>) -> Option<String> {
    if let Some(path) = env_var(file)
        && let Ok(v) = std::fs::read_to_string(&path)
    {
        return Some(v.trim().to_string());
    }
    if let Some(v) = env_var(inline) {
        return Some(v);
    }
    let name = credential?;
    let dir = env_var("CREDENTIALS_DIRECTORY")?;
    std::fs::read_to_string(PathBuf::from(dir).join(name))
        .ok()
        .map(|v| v.trim().to_string())
}

impl EntraConfig {
    pub fn from_env() -> Result<Self, EntraError> {
        let missing = |k: &str| EntraError::Config(format!("{k} is not set"));
        Ok(Self {
            cdp_url: env_var("VK_ENTRA_CDP_URL")
                .ok_or_else(|| missing("VK_ENTRA_CDP_URL"))?
                .trim_end_matches('/')
                .to_string(),
            cdp_token: env_secret("VK_ENTRA_CDP_TOKEN", "VK_ENTRA_CDP_TOKEN_FILE", None)
                .ok_or_else(|| missing("VK_ENTRA_CDP_TOKEN"))?,
            tenant_id: env_var("VK_ENTRA_TENANT_ID")
                .ok_or_else(|| missing("VK_ENTRA_TENANT_ID"))?,
            email: env_var("VK_ENTRA_EMAIL").ok_or_else(|| missing("VK_ENTRA_EMAIL"))?,
            browser_profile: env_var("VK_ENTRA_BROWSER_PROFILE")
                .unwrap_or_else(|| "vk-entra".to_string()),
            op: OnePasswordRef {
                host: env_var("OP_CONNECT_HOST")
                    .ok_or_else(|| missing("OP_CONNECT_HOST"))?
                    .trim_end_matches('/')
                    .to_string(),
                token: env_secret(
                    "OP_CONNECT_TOKEN",
                    "OP_CONNECT_TOKEN_FILE",
                    Some("op-connect-token"),
                )
                .ok_or_else(|| missing("OP_CONNECT_TOKEN"))?,
                vault: env_var("VK_ENTRA_OP_VAULT").ok_or_else(|| missing("VK_ENTRA_OP_VAULT"))?,
                item: env_var("VK_ENTRA_OP_ITEM").ok_or_else(|| missing("VK_ENTRA_OP_ITEM"))?,
                password_field: env_var("VK_ENTRA_OP_PASSWORD_FIELD")
                    .unwrap_or_else(|| "password".to_string()),
                totp_field: env_var("VK_ENTRA_OP_TOTP_FIELD")
                    .ok_or_else(|| missing("VK_ENTRA_OP_TOTP_FIELD"))?,
            },
        })
    }

    fn authorize_endpoint(&self) -> String {
        format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/authorize",
            self.tenant_id
        )
    }

    fn token_endpoint(&self) -> String {
        format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
            self.tenant_id
        )
    }
}

/// A minted delegated token. `refresh_token` is what makes the result durable —
/// the tools renew from it long after the access token expires.
#[derive(Debug, Clone, Deserialize)]
pub struct MintedToken {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_in: i64,
    #[serde(default)]
    pub scope: String,
}

/// Human-readable progress, streamed to the settings UI's login terminal.
#[derive(Clone)]
pub struct Progress(Option<UnboundedSender<String>>);

impl Progress {
    pub fn new(tx: UnboundedSender<String>) -> Self {
        Self(Some(tx))
    }
    pub fn silent() -> Self {
        Self(None)
    }
    pub fn say(&self, msg: impl AsRef<str>) {
        if let Some(tx) = &self.0 {
            let _ = tx.send(format!("{}\r\n", msg.as_ref()));
        }
    }
}

// ---------------------------------------------------------------------------
// 1Password Connect
// ---------------------------------------------------------------------------

/// Fetch one field from the configured Connect item. Returns the field's
/// computed `totp` when present, otherwise its `value`.
async fn op_field(
    http: &reqwest::Client,
    op: &OnePasswordRef,
    field_id: &str,
) -> Result<String, EntraError> {
    let url = format!("{}/v1/vaults/{}/items/{}", op.host, op.vault, op.item);
    let resp = http
        .get(&url)
        .bearer_auth(&op.token)
        .send()
        .await
        .map_err(|e| EntraError::OnePassword(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(EntraError::OnePassword(format!(
            "item fetch returned {}",
            resp.status()
        )));
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| EntraError::OnePassword(e.to_string()))?;
    let fields = body
        .get("fields")
        .and_then(|f| f.as_array())
        .ok_or_else(|| EntraError::OnePassword("item has no fields".into()))?;
    for f in fields {
        if f.get("id").and_then(|v| v.as_str()) == Some(field_id) {
            // Connect returns the current code on `totp` for OTP fields.
            if let Some(totp) = f.get("totp").and_then(|v| v.as_str())
                && !totp.is_empty()
            {
                return Ok(totp.to_string());
            }
            if let Some(value) = f.get("value").and_then(|v| v.as_str()) {
                return Ok(value.to_string());
            }
        }
    }
    Err(EntraError::OnePassword(format!(
        "field {field_id} not found on item"
    )))
}

// ---------------------------------------------------------------------------
// Browser service session
// ---------------------------------------------------------------------------

/// One browser session on the firecrawl service, bound to the persistent
/// Entra profile. Dropping it does not close the session — call `close()`,
/// which is also what persists the profile.
struct BrowserSession<'a> {
    cfg: &'a EntraConfig,
    http: reqwest::Client,
    id: String,
}

impl<'a> BrowserSession<'a> {
    async fn open(
        cfg: &'a EntraConfig,
        http: reqwest::Client,
        save: bool,
    ) -> Result<Self, EntraError> {
        let body = serde_json::json!({
            "profile": { "name": cfg.browser_profile, "saveChanges": save },
            "url": "about:blank",
            "ttl": SESSION_TTL_SECS,
            "activityTtl": SESSION_TTL_SECS,
        });
        let resp = http
            .post(format!("{}/v2/interact", cfg.cdp_url))
            .bearer_auth(&cfg.cdp_token)
            .json(&body)
            .send()
            .await
            .map_err(|e| EntraError::Browser(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(EntraError::Browser(format!(
                "session create returned {}",
                resp.status()
            )));
        }
        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| EntraError::Browser(e.to_string()))?;
        let id = v
            .get("id")
            .and_then(|i| i.as_str())
            .ok_or_else(|| EntraError::Browser("session create returned no id".into()))?
            .to_string();
        Ok(Self { cfg, http, id })
    }

    async fn navigate(&self, url: &str) -> Result<(), EntraError> {
        self.http
            .post(format!(
                "{}/v2/interact/{}/navigate",
                self.cfg.cdp_url, self.id
            ))
            .bearer_auth(&self.cfg.cdp_token)
            .json(&serde_json::json!({ "url": url, "humanize": false }))
            .send()
            .await
            .map_err(|e| EntraError::Browser(e.to_string()))?;
        Ok(())
    }

    /// Run JS in the page (Playwright `page`/`context` in scope) and return its
    /// result. Note that anything registered here — route handlers especially —
    /// lives only for the duration of this one call.
    async fn execute(&self, code: &str) -> Result<serde_json::Value, EntraError> {
        let resp = self
            .http
            .post(format!(
                "{}/v2/interact/{}/execute",
                self.cfg.cdp_url, self.id
            ))
            .bearer_auth(&self.cfg.cdp_token)
            .json(&serde_json::json!({ "code": code }))
            .send()
            .await
            .map_err(|e| EntraError::Browser(e.to_string()))?;
        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| EntraError::Browser(e.to_string()))?;
        if v.get("ok").and_then(|o| o.as_bool()) != Some(true) {
            return Err(EntraError::Browser(format!("execute failed: {v}")));
        }
        Ok(v.get("result").cloned().unwrap_or(serde_json::Value::Null))
    }

    async fn url(&self) -> Result<String, EntraError> {
        Ok(self
            .execute("return page.url()")
            .await?
            .as_str()
            .unwrap_or_default()
            .to_string())
    }

    /// Close the session. For a `saveChanges` session this is what writes the
    /// Entra cookie back to the profile, making later mints silent.
    async fn close(self) {
        let _ = self
            .http
            .delete(format!("{}/v2/interact/{}", self.cfg.cdp_url, self.id))
            .bearer_auth(&self.cfg.cdp_token)
            .send()
            .await;
    }
}

// ---------------------------------------------------------------------------
// Sign-in step machine
// ---------------------------------------------------------------------------

/// Which Entra screen is on-screen right now.
///
/// Detection is driven by page *text*, deliberately: Entra keeps both the
/// `loginfmt` and `passwd` inputs in the DOM and reports both as visible, so
/// probing for elements alone mis-identifies the step and re-submits the email
/// forever.
#[derive(Debug, PartialEq, Eq)]
enum Step {
    Password,
    Totp,
    StaySignedIn,
    AccountPicker,
    Email,
    Unknown,
}

const PROBE_JS: &str = r#"
const txt = (await page.locator('body').innerText().catch(() => '')).toLowerCase();
const vis = async (s) => { const e = await page.$(s); return !!(e && await e.isVisible().catch(() => false)); };
return {
  url: page.url(),
  txt: txt.slice(0, 900),
  hasEmail: await vis('input[name="loginfmt"]'),
  hasOtc:   await vis('input[name="otc"]'),
};
"#;

fn classify(txt: &str, has_email: bool, has_otc: bool) -> Step {
    if txt.contains("enter password") || txt.contains("enter your password") {
        Step::Password
    } else if has_otc
        && (txt.contains("enter code")
            || txt.contains("enter the code")
            || txt.contains("verification code"))
    {
        Step::Totp
    } else if txt.contains("stay signed in") {
        Step::StaySignedIn
    } else if txt.contains("pick an account") || txt.contains("choose an account") {
        Step::AccountPicker
    } else if has_email {
        Step::Email
    } else {
        Step::Unknown
    }
}

fn js_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

// ---------------------------------------------------------------------------
// Minting
// ---------------------------------------------------------------------------

fn pkce_pair() -> (String, String) {
    let mut raw = [0u8; 40];
    rand::thread_rng().fill_bytes(&mut raw);
    let verifier = B64URL.encode(raw);
    let challenge = B64URL.encode(Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

fn authorize_url(
    cfg: &EntraConfig,
    client_id: &str,
    scope: &str,
    challenge: &str,
    silent: bool,
) -> String {
    let mut q: Vec<(&str, String)> = vec![
        ("client_id", client_id.to_string()),
        ("response_type", "code".to_string()),
        ("redirect_uri", NATIVE_CLIENT_REDIRECT.to_string()),
        ("scope", format!("{scope} offline_access openid profile")),
        ("code_challenge", challenge.to_string()),
        ("code_challenge_method", "S256".to_string()),
        ("state", B64URL.encode(rand::random::<[u8; 8]>())),
    ];
    if silent {
        q.push(("prompt", "none".to_string()));
    } else {
        // login_hint steers the tenant-less flows; without it Entra can land on
        // the consumer-account sign-in instead of the work account.
        q.push(("prompt", "select_account".to_string()));
        q.push(("login_hint", cfg.email.clone()));
    }
    let qs = q
        .into_iter()
        .map(|(k, v)| format!("{k}={}", urlencode(&v)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{}?{qs}", cfg.authorize_endpoint())
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Pull `code` (or an `error`) out of a redirect URL's query string.
fn query_params(url: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Some(q) = url.split_once('?').map(|(_, q)| q) {
        for pair in q.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                map.insert(k.to_string(), urldecode(v));
            }
        }
    }
    map
}

fn urldecode(s: &str) -> String {
    let bytes = s.replace('+', " ").into_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(b) = u8::from_str_radix(&String::from_utf8_lossy(&bytes[i + 1..i + 3]), 16)
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Mint a delegated token for `client_id` covering `scope`.
///
/// Tries the silent path first (`prompt=none` against the saved profile). Only
/// if Entra says interaction is required does it spend the password and TOTP.
pub async fn mint(
    cfg: &EntraConfig,
    client_id: &str,
    scope: &str,
    progress: &Progress,
) -> Result<MintedToken, EntraError> {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(90))
        .build()
        .map_err(|e| EntraError::Browser(e.to_string()))?;

    let (verifier, challenge) = pkce_pair();
    let session = BrowserSession::open(cfg, http.clone(), true).await?;
    progress.say(format!(
        "Opened browser session on profile '{}'.",
        cfg.browser_profile
    ));

    let result = async {
        // --- silent attempt ---------------------------------------------
        progress.say("Trying silent sign-in against the saved session…");
        session
            .navigate(&authorize_url(cfg, client_id, scope, &challenge, true))
            .await?;
        let mut code = None;
        for _ in 0..15 {
            let url = session.url().await?;
            if url.starts_with(NATIVE_CLIENT_REDIRECT) {
                let q = query_params(&url);
                if let Some(c) = q.get("code") {
                    progress.say("Silent sign-in succeeded — no credentials needed.");
                    code = Some(c.clone());
                }
                break;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        // --- interactive fallback ---------------------------------------
        if code.is_none() {
            progress.say("Silent sign-in unavailable; signing in interactively.");
            session
                .navigate(&authorize_url(cfg, client_id, scope, &challenge, false))
                .await?;
            code = Some(interactive_sign_in(cfg, &http, &session, progress).await?);
        }

        let code = code.ok_or_else(|| EntraError::Auth("no authorization code".into()))?;
        progress.say("Redeeming the authorization code…");
        redeem(cfg, &http, client_id, &code, &verifier).await
    }
    .await;

    session.close().await;
    progress.say("Browser session closed (profile saved).");
    result
}

async fn interactive_sign_in(
    cfg: &EntraConfig,
    http: &reqwest::Client,
    session: &BrowserSession<'_>,
    progress: &Progress,
) -> Result<String, EntraError> {
    let deadline = tokio::time::Instant::now() + INTERACTIVE_TIMEOUT;
    let mut last_step: Option<String> = None;
    while tokio::time::Instant::now() < deadline {
        let probe = session.execute(PROBE_JS).await?;
        let url = probe
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if url.starts_with(NATIVE_CLIENT_REDIRECT) {
            let q = query_params(url);
            if let Some(code) = q.get("code") {
                return Ok(code.clone());
            }
            let err = q.get("error_description").or_else(|| q.get("error"));
            return Err(EntraError::Auth(
                err.cloned().unwrap_or_else(|| "sign-in failed".into()),
            ));
        }
        let txt = probe
            .get("txt")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let step = classify(
            txt,
            probe.get("hasEmail").and_then(|v| v.as_bool()) == Some(true),
            probe.get("hasOtc").and_then(|v| v.as_bool()) == Some(true),
        );
        let label = format!("{step:?}");
        if last_step.as_deref() != Some(label.as_str()) {
            progress.say(format!("  sign-in step: {label}"));
            last_step = Some(label);
        }
        match step {
            Step::Password => {
                let pw = op_field(http, &cfg.op, &cfg.op.password_field).await?;
                session
                    .execute(&format!(
                        "await page.fill('input[name=\"passwd\"]', {}); \
                         await page.click('#idSIButton9'); return 1",
                        js_string(&pw)
                    ))
                    .await?;
            }
            Step::Totp => {
                // Fetched just in time: the code rotates every 30s.
                let code = op_field(http, &cfg.op, &cfg.op.totp_field).await?;
                session
                    .execute(&format!(
                        "await page.fill('input[name=\"otc\"]', {}); \
                         await page.click('#idSubmit_SAOTCC_Continue, #idSIButton9'); return 1",
                        js_string(&code)
                    ))
                    .await?;
            }
            Step::StaySignedIn => {
                // "Yes" here is what makes the profile cookie persistent, and
                // therefore what makes every later mint silent.
                session
                    .execute("await page.click('#idSIButton9'); return 1")
                    .await?;
            }
            Step::AccountPicker => {
                session
                    .execute(&format!(
                        "await page.click('text=' + {}).catch(() => {{}}); return 1",
                        js_string(&cfg.email)
                    ))
                    .await?;
            }
            Step::Email => {
                session
                    .execute(&format!(
                        "await page.fill('input[name=\"loginfmt\"]', {}); \
                         await page.click('#idSIButton9'); return 1",
                        js_string(&cfg.email)
                    ))
                    .await?;
            }
            Step::Unknown => {}
        }
        tokio::time::sleep(STEP_INTERVAL).await;
    }
    Err(EntraError::Auth("interactive sign-in timed out".into()))
}

async fn redeem(
    cfg: &EntraConfig,
    http: &reqwest::Client,
    client_id: &str,
    code: &str,
    verifier: &str,
) -> Result<MintedToken, EntraError> {
    // Encoded by hand: this build of reqwest is compiled without the form
    // feature, so `.form()` is unavailable.
    let body = [
        ("client_id", client_id),
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", NATIVE_CLIENT_REDIRECT),
        ("code_verifier", verifier),
    ]
    .iter()
    .map(|(k, v)| format!("{k}={}", urlencode(v)))
    .collect::<Vec<_>>()
    .join("&");
    let resp = http
        .post(cfg.token_endpoint())
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|e| EntraError::Auth(e.to_string()))?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| EntraError::Auth(e.to_string()))?;
    if !status.is_success() {
        return Err(EntraError::Auth(format!("token endpoint {status}: {body}")));
    }
    serde_json::from_str(&body).map_err(|e| EntraError::Auth(format!("malformed token: {e}")))
}

// ---------------------------------------------------------------------------
// Applying a minted token to a specific tool
// ---------------------------------------------------------------------------

/// Scope `az` itself requests. Keeping it identical matters: the refresh token
/// is cached under this target, and `az` looks it up by the same string.
pub const AZ_SCOPE: &str = "https://management.core.windows.net//.default";
pub const AZ_CLIENT_ID: &str = "04b07795-8ddb-461a-bbee-02f9e1bf7b46";
pub const GRAPH_CLIENT_ID: &str = "14d82eec-204b-4c2f-b7e8-296a70dab67e";
pub const GRAPH_SCOPE: &str = "https://graph.microsoft.com/.default";

/// Populate `az`'s own MSAL cache from a minted refresh token.
///
/// The cache is written by MSAL itself rather than hand-rolled: we hand the
/// refresh token to the `msal` in az's bundled venv and let it serialise the
/// result. That keeps us correct across MSAL cache-schema changes, which a
/// hand-written file would not be.
pub async fn apply_az(
    token: &MintedToken,
    venv_python: &std::path::Path,
    config_dir: &std::path::Path,
    cfg: &EntraConfig,
    progress: &Progress,
) -> Result<(), EntraError> {
    let refresh = token
        .refresh_token
        .as_deref()
        .ok_or_else(|| EntraError::Auth("Entra returned no refresh token for az".into()))?;
    progress.say("Writing az's MSAL token cache…");

    let input = serde_json::json!({
        "refresh_token": refresh,
        "tenant_id": cfg.tenant_id,
        "client_id": AZ_CLIENT_ID,
        "scope": AZ_SCOPE,
        "config_dir": config_dir.to_string_lossy(),
    })
    .to_string();

    let mut child = tokio::process::Command::new(venv_python)
        .arg("-c")
        .arg(AZ_INJECT_PY)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    {
        use tokio::io::AsyncWriteExt;
        // via stdin, never argv — argv is world-readable in /proc
        let mut stdin = child.stdin.take().expect("piped");
        stdin.write_all(input.as_bytes()).await?;
        stdin.shutdown().await?;
    }
    let out = child.wait_with_output().await?;
    if !out.status.success() {
        return Err(EntraError::Auth(format!(
            "az cache injection failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        progress.say(format!("  {line}"));
    }
    Ok(())
}

/// Python run inside az's own venv (which already has `msal`).
const AZ_INJECT_PY: &str = r#"
import json, os, sys, urllib.request
import msal

cfg = json.load(sys.stdin)
config_dir = cfg["config_dir"]
os.makedirs(config_dir, exist_ok=True)
cache_path = os.path.join(config_dir, "msal_token_cache.json")

cache = msal.SerializableTokenCache()
if os.path.exists(cache_path):
    with open(cache_path) as f:
        cache.deserialize(f.read())

app = msal.PublicClientApplication(
    cfg["client_id"],
    authority="https://login.microsoftonline.com/%s" % cfg["tenant_id"],
    token_cache=cache,
)
res = app.acquire_token_by_refresh_token(cfg["refresh_token"], scopes=[cfg["scope"]])
if "access_token" not in res:
    sys.stderr.write(json.dumps(res)[:400]); sys.exit(1)

with open(cache_path, "w") as f:
    f.write(cache.serialize())
os.chmod(cache_path, 0o600)
print("wrote msal_token_cache.json")

# az refuses to run without a subscription profile, so build one from ARM.
req = urllib.request.Request(
    "https://management.azure.com/subscriptions?api-version=2020-01-01",
    headers={"Authorization": "Bearer " + res["access_token"]},
)
subs = json.load(urllib.request.urlopen(req, timeout=30))["value"]
account = app.get_accounts()[0]
profile = {"subscriptions": [], "installationId": os.urandom(16).hex()}
for i, s in enumerate(subs):
    profile["subscriptions"].append({
        "id": s["subscriptionId"], "name": s.get("displayName"),
        "state": s.get("state", "Enabled"),
        "user": {"name": account["username"], "type": "user"},
        "isDefault": i == 0, "tenantId": s.get("tenantId"),
        "environmentName": "AzureCloud", "homeTenantId": s.get("tenantId"),
        "managedByTenants": [{"tenantId": t["tenantId"]} for t in s.get("managedByTenants", [])],
    })
# az writes this file with a BOM and fails to parse it without one.
with open(os.path.join(config_dir, "azureProfile.json"), "w", encoding="utf-8-sig") as f:
    json.dump(profile, f)
print("wrote azureProfile.json with %d subscription(s)" % len(subs))
"#;

// ---------------------------------------------------------------------------
// Local-browser sign-in (for tools that run their own OAuth flow)
// ---------------------------------------------------------------------------

/// Toolchain for the local-browser path, supplied by Nix rather than guessed.
pub struct LocalBrowserTools {
    pub chromium: PathBuf,
}

impl LocalBrowserTools {
    pub fn from_env() -> Result<Self, EntraError> {
        Ok(Self {
            chromium: env_var("VK_ENTRA_CHROMIUM")
                .map(PathBuf::from)
                .ok_or_else(|| EntraError::Config("VK_ENTRA_CHROMIUM is not set".into()))?,
        })
    }
}

/// Export the Entra cookies held by the shared browser profile.
///
/// This is what lets the local browser skip the sign-in entirely: it inherits
/// the session the profile already holds, so no password or TOTP is spent.
async fn export_profile_cookies(
    cfg: &EntraConfig,
    http: &reqwest::Client,
) -> Result<Vec<serde_json::Value>, EntraError> {
    let session = BrowserSession::open(cfg, http.clone(), false).await?;
    let result = session.execute("return await context.cookies()").await;
    session.close().await;
    Ok(result?.as_array().cloned().unwrap_or_default())
}

/// The bits of Chrome DevTools Protocol this needs, and nothing more.
struct LocalChrome {
    child: tokio::process::Child,
    ws: futures::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        tokio_tungstenite::tungstenite::Message,
    >,
    rx: tokio::sync::mpsc::UnboundedReceiver<serde_json::Value>,
    session_id: String,
    next_id: i64,
}

impl LocalChrome {
    /// Launch headless chromium on an ephemeral profile and attach to a tab.
    async fn launch(
        tools: &LocalBrowserTools,
        port: u16,
        dir: &std::path::Path,
    ) -> Result<Self, EntraError> {
        use futures::StreamExt;

        let child = tokio::process::Command::new(&tools.chromium)
            .args([
                "--headless=new",
                &format!("--remote-debugging-port={port}"),
                &format!("--user-data-dir={}", dir.display()),
                "--no-sandbox",
                "--disable-dev-shm-usage",
                "--disable-gpu",
                "about:blank",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()?;

        // Poll for the debugger endpoint; chromium takes a moment to bind.
        let http = reqwest::Client::new();
        let mut ws_url = None;
        for _ in 0..40 {
            tokio::time::sleep(Duration::from_millis(500)).await;
            if let Ok(resp) = http
                .get(format!("http://127.0.0.1:{port}/json/version"))
                .send()
                .await
                && let Ok(v) = resp.json::<serde_json::Value>().await
                && let Some(u) = v.get("webSocketDebuggerUrl").and_then(|u| u.as_str())
            {
                ws_url = Some(u.to_string());
                break;
            }
        }
        let ws_url =
            ws_url.ok_or_else(|| EntraError::Browser("chromium never exposed CDP".into()))?;
        let (stream, _) = tokio_tungstenite::connect_async(&ws_url)
            .await
            .map_err(|e| EntraError::Browser(format!("CDP connect: {e}")))?;
        let (sink, mut source) = stream.split();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            while let Some(Ok(msg)) = source.next().await {
                if let Ok(text) = msg.into_text()
                    && let Ok(v) = serde_json::from_str::<serde_json::Value>(&text)
                {
                    let _ = tx.send(v);
                }
            }
        });

        let mut me = Self {
            child,
            ws: sink,
            rx,
            session_id: String::new(),
            next_id: 0,
        };
        let target = me
            .call(
                "Target.createTarget",
                serde_json::json!({"url": "about:blank"}),
            )
            .await?;
        let target_id = target
            .get("targetId")
            .and_then(|t| t.as_str())
            .ok_or_else(|| EntraError::Browser("no targetId".into()))?
            .to_string();
        let attached = me
            .call(
                "Target.attachToTarget",
                serde_json::json!({"targetId": target_id, "flatten": true}),
            )
            .await?;
        me.session_id = attached
            .get("sessionId")
            .and_then(|s| s.as_str())
            .ok_or_else(|| EntraError::Browser("no sessionId".into()))?
            .to_string();
        me.call("Network.enable", serde_json::json!({})).await?;
        me.call("Page.enable", serde_json::json!({})).await?;
        Ok(me)
    }

    async fn call(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, EntraError> {
        use futures::SinkExt;
        self.next_id += 1;
        let id = self.next_id;
        let mut msg = serde_json::json!({"id": id, "method": method, "params": params});
        if !self.session_id.is_empty() {
            msg["sessionId"] = serde_json::Value::String(self.session_id.clone());
        }
        self.ws
            .send(tokio_tungstenite::tungstenite::Message::Text(
                msg.to_string().into(),
            ))
            .await
            .map_err(|e| EntraError::Browser(format!("CDP send: {e}")))?;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout_at(deadline, self.rx.recv()).await {
                Ok(Some(v)) => {
                    if v.get("id").and_then(|i| i.as_i64()) == Some(id) {
                        if let Some(err) = v.get("error") {
                            return Err(EntraError::Browser(format!("{method}: {err}")));
                        }
                        return Ok(v.get("result").cloned().unwrap_or(serde_json::Value::Null));
                    }
                }
                _ => break,
            }
        }
        Err(EntraError::Browser(format!("{method}: timed out")))
    }

    /// Transplant the shared profile's cookies so Entra recognises the session.
    async fn seed_cookies(&mut self, cookies: &[serde_json::Value]) -> Result<usize, EntraError> {
        let mapped: Vec<serde_json::Value> = cookies
            .iter()
            .map(|c| {
                let mut o = serde_json::json!({
                    "name": c.get("name").cloned().unwrap_or_default(),
                    "value": c.get("value").cloned().unwrap_or_default(),
                    "domain": c.get("domain").cloned().unwrap_or_default(),
                    "path": c.get("path").cloned().unwrap_or_else(|| "/".into()),
                    "secure": c.get("secure").and_then(|v| v.as_bool()).unwrap_or(false),
                    "httpOnly": c.get("httpOnly").and_then(|v| v.as_bool()).unwrap_or(false),
                });
                // A negative/zero expiry means "session cookie"; CDP wants the
                // field omitted rather than a sentinel.
                if let Some(exp) = c.get("expires").and_then(|v| v.as_f64())
                    && exp > 0.0
                {
                    o["expires"] = serde_json::json!(exp);
                }
                if let Some(ss) = c.get("sameSite").and_then(|v| v.as_str())
                    && matches!(ss, "Strict" | "Lax" | "None")
                {
                    o["sameSite"] = serde_json::json!(ss);
                }
                o
            })
            .collect();
        let n = mapped.len();
        self.call(
            "Network.setCookies",
            serde_json::json!({ "cookies": mapped }),
        )
        .await?;
        Ok(n)
    }

    async fn navigate(&mut self, url: &str) -> Result<(), EntraError> {
        self.call("Page.navigate", serde_json::json!({ "url": url }))
            .await?;
        Ok(())
    }

    /// Click the account tile if a picker rendered; otherwise report the URL.
    async fn nudge(&mut self, email: &str) -> Result<String, EntraError> {
        let expr = format!(
            "(() => {{ const email = {}; \
             const tile = [...document.querySelectorAll('div,small,button')].find(e => e.textContent.trim() === email); \
             if (tile) {{ tile.click(); return 'account tile'; }} \
             const next = document.querySelector('#idSIButton9'); \
             if (next) {{ next.click(); return 'continue'; }} \
             return 'waiting'; }})()",
            js_string(email)
        );
        let r = self
            .call(
                "Runtime.evaluate",
                serde_json::json!({"expression": expr, "returnByValue": true}),
            )
            .await?;
        Ok(r.get("result")
            .and_then(|v| v.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_string())
    }
}

/// Run a tool's *own* browser sign-in, supplying a browser that can reach the
/// loopback listener it opens.
///
/// The remote browser cannot: it runs in a sandbox with no route to this host,
/// which is why this path exists at all. A local chromium can, so the tool's
/// redirect completes normally and the tool writes its own native credential
/// store — no cache format to reverse-engineer, and nothing to intercept.
#[allow(clippy::too_many_arguments)]
pub async fn native_browser_login(
    cfg: &EntraConfig,
    tools: &LocalBrowserTools,
    executable: &std::path::Path,
    args: &[String],
    progress: &Progress,
) -> Result<(), EntraError> {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(90))
        .build()
        .map_err(|e| EntraError::Browser(e.to_string()))?;

    progress.say("Exporting the Entra session from the shared browser profile…");
    let cookies = export_profile_cookies(cfg, &http).await?;
    progress.say(format!("  {} cookies", cookies.len()));

    // Scratch space: a shim that captures the URL the tool wants to open, and
    // an ephemeral chromium profile.
    let work = std::env::temp_dir().join(format!("vk-entra-{}", uuid::Uuid::new_v4()));
    let shim_dir = work.join("shim");
    let chrome_dir = work.join("chrome");
    std::fs::create_dir_all(&shim_dir)?;
    std::fs::create_dir_all(&chrome_dir)?;
    let url_file = work.join("auth-url");
    let shim = shim_dir.join("xdg-open");
    std::fs::write(
        &shim,
        format!(
            "#!/bin/sh\nprintf '%s' \"$1\" > {}\nexit 0\n",
            url_file.display()
        ),
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755))?;
    }

    let result = async {
        progress.say("Starting the tool's own sign-in…");
        let path = format!(
            "{}:{}",
            shim_dir.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let mut child = tokio::process::Command::new(executable)
            .args(args)
            .env("PATH", path)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        // Wait for the tool to hand us its authorize URL.
        let mut auth = None;
        for _ in 0..60 {
            tokio::time::sleep(Duration::from_secs(1)).await;
            if let Ok(v) = std::fs::read_to_string(&url_file)
                && !v.trim().is_empty()
            {
                auth = Some(v.trim().to_string());
                break;
            }
            if child.try_wait()?.is_some() {
                break;
            }
        }
        let auth =
            auth.ok_or_else(|| EntraError::Auth("the tool never opened a sign-in URL".into()))?;

        // MSAL asks for select_account, which parks on the picker even with a
        // live session. Drop it and hint the account so Entra goes straight
        // through; the redirect then lands on the tool's own listener.
        let auth = rewrite_authorize_url(&auth, &cfg.email);
        progress.say("Completing it in a local browser (it can reach the tool's listener)…");

        let port = 9333;
        let mut chrome = LocalChrome::launch(tools, port, &chrome_dir).await?;
        let n = chrome.seed_cookies(&cookies).await?;
        progress.say(format!("  seeded {n} cookies into the local browser"));
        chrome.navigate(&auth).await?;

        let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
        let mut status = None;
        while tokio::time::Instant::now() < deadline {
            if let Some(s) = child.try_wait()? {
                status = Some(s);
                break;
            }
            tokio::time::sleep(Duration::from_millis(1500)).await;
            let _ = chrome.nudge(&cfg.email).await;
        }
        let _ = chrome.child.kill().await;

        let status = match status {
            Some(s) => s,
            None => {
                let _ = child.kill().await;
                return Err(EntraError::Auth("the tool's sign-in timed out".into()));
            }
        };
        if !status.success() {
            let out = child.wait_with_output().await?;
            let tail = String::from_utf8_lossy(&out.stdout);
            return Err(EntraError::Auth(format!(
                "the tool reported: {}",
                tail.trim()
                    .chars()
                    .rev()
                    .take(300)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect::<String>()
            )));
        }
        progress.say("The tool completed its sign-in and wrote its own credential store.");
        Ok(())
    }
    .await;

    let _ = std::fs::remove_dir_all(&work);
    result
}

/// Strip MSAL's `prompt=select_account` and hint the account, so an existing
/// session resolves without rendering a picker.
fn rewrite_authorize_url(url: &str, email: &str) -> String {
    let (base, query) = match url.split_once('?') {
        Some((b, q)) => (b, q),
        None => return url.to_string(),
    };
    let mut parts: Vec<String> = query
        .split('&')
        .filter(|p| {
            let key = p.split_once('=').map(|(k, _)| k).unwrap_or(p);
            key != "prompt" && key != "login_hint"
        })
        .map(|s| s.to_string())
        .collect();
    parts.push(format!("login_hint={}", urlencode(email)));
    format!("{base}?{}", parts.join("&"))
}

/// Marker delimiting the block vibe-kanban owns inside the PowerShell profile.
const PS_BLOCK_BEGIN: &str = "# >>> vibe-kanban Entra auth >>>";
const PS_BLOCK_END: &str = "# <<< vibe-kanban Entra auth <<<";

/// Persist a minted Graph token and make `graph-powershell` connect with it.
///
/// `Connect-MgGraph -AccessToken` only authenticates the current process, so a
/// durable login needs the connect to happen at pwsh startup. We write the
/// token (with its refresh token) to a private file and add a vk-owned block to
/// the PowerShell profile that connects from it, refreshing first when the
/// access token has aged out.
///
/// Caveat, by design of pwsh: `-NoProfile` skips profiles, so an invocation
/// using it starts unauthenticated.
pub async fn apply_graph_powershell(
    token: &MintedToken,
    token_path: &std::path::Path,
    profile_path: &std::path::Path,
    cfg: &EntraConfig,
    progress: &Progress,
) -> Result<(), EntraError> {
    let refresh = token.refresh_token.as_deref().unwrap_or_default();
    let expires_at = chrono::Utc::now().timestamp() + token.expires_in.max(0);
    let doc = serde_json::json!({
        "access_token": token.access_token,
        "refresh_token": refresh,
        "expires_at": expires_at,
        "client_id": GRAPH_CLIENT_ID,
        "scope": format!("{GRAPH_SCOPE} offline_access openid profile"),
        "token_endpoint": cfg.token_endpoint(),
    });
    if let Some(dir) = token_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(
        token_path,
        serde_json::to_vec_pretty(&doc).unwrap_or_default(),
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(token_path, std::fs::Permissions::from_mode(0o600))?;
    }
    progress.say("Stored the Graph token for PowerShell.");

    if let Some(dir) = profile_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let existing = std::fs::read_to_string(profile_path).unwrap_or_default();
    let updated = replace_ps_block(&existing, &ps_auth_block(token_path));
    std::fs::write(profile_path, updated)?;
    progress.say(format!(
        "Updated the PowerShell profile at {} (note: -NoProfile bypasses it).",
        profile_path.display()
    ));
    Ok(())
}

/// Swap vibe-kanban's block into a profile, leaving everything else untouched.
fn replace_ps_block(existing: &str, block: &str) -> String {
    let stripped = match (existing.find(PS_BLOCK_BEGIN), existing.find(PS_BLOCK_END)) {
        (Some(start), Some(end)) if end > start => {
            let tail = &existing[end + PS_BLOCK_END.len()..];
            format!("{}{}", &existing[..start], tail.trim_start_matches('\n'))
        }
        _ => existing.to_string(),
    };
    let mut out = stripped.trim_end().to_string();
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(block);
    out.push('\n');
    out
}

fn ps_auth_block(token_path: &std::path::Path) -> String {
    let quoted = token_path.display().to_string().replace('\'', "''");
    format!(
        r#"{PS_BLOCK_BEGIN}
# Generated by vibe-kanban; edits inside this block are overwritten.
$VkGraphTokenFile = '{quoted}'
if (Test-Path $VkGraphTokenFile) {{
  try {{
    $vk = Get-Content $VkGraphTokenFile -Raw | ConvertFrom-Json
    $now = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
    if ($now -ge ($vk.expires_at - 120) -and $vk.refresh_token) {{
      $resp = Invoke-RestMethod -Method POST -Uri $vk.token_endpoint -Body @{{
        client_id     = $vk.client_id
        grant_type    = 'refresh_token'
        refresh_token = $vk.refresh_token
        scope         = $vk.scope
      }}
      $vk.access_token = $resp.access_token
      if ($resp.refresh_token) {{ $vk.refresh_token = $resp.refresh_token }}
      $vk.expires_at = $now + [int]$resp.expires_in
      $vk | ConvertTo-Json | Set-Content $VkGraphTokenFile
    }}
    Connect-MgGraph -AccessToken ($vk.access_token | ConvertTo-SecureString -AsPlainText -Force) -NoWelcome
  }} catch {{
    Write-Verbose "vibe-kanban: Graph auto-connect failed: $_"
  }}
}}
{PS_BLOCK_END}"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorize_rewrite_drops_the_picker_and_hints_the_account() {
        let url = "https://login.microsoftonline.com/common/oauth2/v2.0/authorize                   ?scope=User.Read&redirect_uri=http%3A%2F%2Flocalhost%3A1234                   &prompt=select_account&state=abc";
        let out = rewrite_authorize_url(url, "a@b.com");
        // select_account parks on a picker even with a live session cookie.
        assert!(!out.contains("prompt="), "prompt must be dropped: {out}");
        assert!(out.contains("login_hint=a%40b.com"));
        // Everything the tool put there must survive, especially its listener.
        assert!(out.contains("redirect_uri=http%3A%2F%2Flocalhost%3A1234"));
        assert!(out.contains("state=abc"));
        assert!(out.contains("scope=User.Read"));
    }

    #[test]
    fn authorize_rewrite_replaces_an_existing_login_hint() {
        let out = rewrite_authorize_url("https://x/y?login_hint=old%40x.com&a=1", "new@x.com");
        assert!(out.contains("login_hint=new%40x.com"));
        assert!(!out.contains("old%40x.com"));
        assert_eq!(out.matches("login_hint=").count(), 1);
    }

    #[test]
    fn ps_block_is_idempotent_and_preserves_user_content() {
        let user = "# my profile\nSet-Alias ll Get-ChildItem\n";
        let once = replace_ps_block(user, &ps_auth_block(std::path::Path::new("/t/tok.json")));
        assert!(once.contains("Set-Alias ll Get-ChildItem"));
        assert_eq!(once.matches(PS_BLOCK_BEGIN).count(), 1);
        let twice = replace_ps_block(&once, &ps_auth_block(std::path::Path::new("/t/tok.json")));
        assert_eq!(
            twice.matches(PS_BLOCK_BEGIN).count(),
            1,
            "must not stack blocks"
        );
        assert!(twice.contains("Set-Alias ll Get-ChildItem"));
        assert_eq!(
            once, twice,
            "rewriting an unchanged block should be a no-op"
        );
    }

    #[test]
    fn ps_block_replacement_swaps_the_token_path() {
        let first = replace_ps_block("", &ps_auth_block(std::path::Path::new("/a/one.json")));
        let second = replace_ps_block(&first, &ps_auth_block(std::path::Path::new("/b/two.json")));
        assert!(second.contains("/b/two.json"));
        assert!(!second.contains("/a/one.json"));
    }

    #[test]
    fn classifies_password_step_even_though_email_input_is_still_visible() {
        // Entra leaves `loginfmt` in the DOM and reporting visible on the
        // password screen; element probing alone would loop on the email step.
        let step = classify(
            "david@example.com enter password forgot my password",
            true,
            false,
        );
        assert_eq!(step, Step::Password);
    }

    #[test]
    fn classifies_totp_only_when_the_code_box_is_present() {
        assert_eq!(
            classify("enter code from the authenticator app", true, true),
            Step::Totp
        );
        // Same words, no input rendered yet -> not the TOTP step.
        assert_ne!(classify("enter code", true, false), Step::Totp);
    }

    #[test]
    fn classifies_picker_and_email() {
        assert_eq!(
            classify("pick an account", false, false),
            Step::AccountPicker
        );
        assert_eq!(classify("sign in", true, false), Step::Email);
        assert_eq!(classify("", false, false), Step::Unknown);
    }

    #[test]
    fn pkce_challenge_is_the_sha256_of_the_verifier() {
        let (verifier, challenge) = pkce_pair();
        assert_eq!(
            challenge,
            B64URL.encode(Sha256::digest(verifier.as_bytes()))
        );
        assert!(
            verifier.len() >= 43,
            "verifier must be >= 43 chars per RFC 7636"
        );
    }

    #[test]
    fn parses_code_and_error_out_of_a_redirect() {
        let q = query_params("https://x/y?code=abc123&state=zz");
        assert_eq!(q.get("code").map(String::as_str), Some("abc123"));
        let q = query_params("https://x/y?error=login_required&error_description=a%20b+c");
        assert_eq!(q.get("error").map(String::as_str), Some("login_required"));
        assert_eq!(
            q.get("error_description").map(String::as_str),
            Some("a b c")
        );
    }

    #[test]
    fn silent_and_interactive_authorize_urls_differ_only_where_intended() {
        let cfg = EntraConfig {
            cdp_url: "http://cdp".into(),
            cdp_token: "t".into(),
            tenant_id: "tid".into(),
            email: "a@b.com".into(),
            browser_profile: "vk-entra".into(),
            op: OnePasswordRef {
                host: "http://op".into(),
                token: "t".into(),
                vault: "v".into(),
                item: "i".into(),
                password_field: "password".into(),
                totp_field: "TOTP".into(),
            },
        };
        let silent = authorize_url(&cfg, "cid", "scope", "chal", true);
        assert!(silent.contains("prompt=none"));
        assert!(!silent.contains("login_hint"));
        let interactive = authorize_url(&cfg, "cid", "scope", "chal", false);
        assert!(interactive.contains("prompt=select_account"));
        assert!(interactive.contains("login_hint=a%40b.com"));
        for url in [&silent, &interactive] {
            assert!(url.contains("code_challenge_method=S256"));
            assert!(url.contains("offline_access"), "need a refresh token");
        }
    }
}
