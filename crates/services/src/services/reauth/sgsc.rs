//! sgsc-mcp gateway: complete a backend's per-user onboarding with no human.
//!
//! When a gateway tool answers `reauth_required` it points at
//! `<gateway>/_sgsc/auth/onboard/<backend>`. That URL identifies the user
//! through Entra (handled by [`entra_mint::drive`] on the shared profile) and
//! then hands off to the backend's provider:
//!
//! * **Snowflake (`dp`)** shows a *Sign in using ENTRA_ID_SSO* button that
//!   reuses the Entra session.
//! * **Salesforce (`sf`)** lands on a native My-Domain login with no SSO
//!   button — but Salesforce is behind MyApps, so opening its MyApps tile
//!   (IdP-initiated SAML) sets the Salesforce session, after which the
//!   onboard URL sails through. No Salesforce password exists or is needed.
//!
//! Success is the gateway's own callback (`/_sgsc/auth/callback/<backend>`)
//! or its "Connected" page. There is no non-mutating status check, which is
//! why these targets are on demand only and never swept.

use std::time::Duration;

use super::ReauthError;
use crate::services::entra_mint::{
    self, BrowserSession, EntraConfig, EntraError, PageVerdict, Probe, Progress,
};

const DRIVE_TIMEOUT: Duration = Duration::from_secs(180);
const MYAPPS_URL: &str = "https://myapps.microsoft.com";

/// The configured onboard URL template (must contain `{backend}`).
pub fn onboard_template() -> Result<String, ReauthError> {
    let key = "VK_SGSC_ONBOARD_URL_TEMPLATE";
    let template = std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .ok_or_else(|| ReauthError::NotConfigured(format!("{key} is not set")))?;
    if !template.starts_with("https://") || !template.contains("{backend}") {
        return Err(ReauthError::NotConfigured(format!(
            "{key} must be an https URL containing {{backend}}"
        )));
    }
    Ok(template)
}

pub(crate) fn onboard_url(template: &str, backend: &str) -> String {
    template.replace("{backend}", backend)
}

fn host_of(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_ascii_lowercase))
        .unwrap_or_default()
}

fn is_salesforce(host: &str) -> bool {
    host.ends_with(".salesforce.com") || host.ends_with(".force.com")
}

/// How a pass over the onboarding flow ended.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Onboard {
    Connected,
    /// Salesforce wants its native login: go through MyApps first.
    NeedsMyApps,
}

const SNOWFLAKE_SSO_JS: &str = r#"
const b = page.getByRole('button', { name: /entra_id_sso|sign in using/i }).first();
if (await b.count()) { await b.click(); return 'sso'; }
return null;
"#;

const SALESFORCE_ALLOW_JS: &str = r#"
const b = page.getByRole('button', { name: /^\s*allow\s*$/i }).first();
if (await b.count()) { await b.click(); return 'allow'; }
return null;
"#;

/// Opens the Salesforce MyApps tile; IdP-initiated SAML usually opens it in a
/// new tab, and that tab's load is what sets the Salesforce session cookie.
const MYAPPS_TILE_JS: &str = r#"
const tile = page.getByText(/^\s*salesforce\s*$/i).first();
if (!(await tile.count())) return 'no-tile';
const [popup] = await Promise.all([
  context.waitForEvent('page', { timeout: 20000 }).catch(() => null),
  tile.click(),
]);
const target = popup || page;
await target.waitForLoadState('load', { timeout: 30000 }).catch(() => {});
await target.waitForTimeout(5000);
if (popup) await popup.close().catch(() => {});
return popup ? 'popup' : 'same-tab';
"#;

/// Gateway copy that means the grant was not stored.
const FAILURE_WORDS: &[&str] = &["not connected", "disconnected", "failed", "denied", "error"];

/// Whether the text affirmatively says "connected" (not "disconnected").
fn says_connected(txt: &str) -> bool {
    txt.match_indices("connected").any(|(at, _)| {
        let before = &txt[..at];
        !before.ends_with("dis") && !before.trim_end().ends_with("not")
    })
}

/// A page on the gateway itself: its own verdict on the onboarding.
fn gateway_verdict(p: &Probe, callback: &str, backend: &str) -> PageVerdict<Onboard> {
    let Ok(url) = url::Url::parse(&p.url) else {
        return PageVerdict::Continue;
    };
    let on_callback = url.path().starts_with(callback);
    let failed = FAILURE_WORDS.iter().any(|w| p.txt.contains(w));
    // An OAuth callback also carries failures; that is the gateway's verdict,
    // so surface it rather than waiting out the timeout.
    if on_callback && let Some((_, error)) = url.query_pairs().find(|(k, _)| k == "error") {
        let detail = url
            .query_pairs()
            .find(|(k, _)| k == "error_description")
            .map(|(_, v)| format!(": {v}"))
            .unwrap_or_default();
        return PageVerdict::Fail(format!("the gateway reported {error}{detail}"));
    }
    if on_callback && failed {
        return PageVerdict::Fail(format!(
            "the gateway did not connect {backend}: {}",
            p.txt.trim().chars().take(200).collect::<String>()
        ));
    }
    // A gateway auth page names its backend as the last path segment; a
    // "connected" page for another backend says nothing about this one.
    let other_backend = url.path().starts_with("/_sgsc/auth/")
        && url.path_segments().and_then(|mut s| s.next_back()) != Some(backend);
    // Success is the gateway saying so, for this backend, not merely
    // arriving somewhere.
    if says_connected(&p.txt) && !failed && !other_backend {
        return PageVerdict::Done(Onboard::Connected);
    }
    PageVerdict::Continue
}

pub(crate) fn sgsc_page_verdict(
    p: &Probe,
    backend: &str,
    gateway_host: &str,
    detour_available: bool,
) -> PageVerdict<Onboard> {
    let host = p.host();
    if host == gateway_host {
        let callback = format!("/_sgsc/auth/callback/{backend}");
        return gateway_verdict(p, &callback, backend);
    }
    if host.ends_with(".snowflakecomputing.com")
        && (p.txt.contains("entra_id_sso") || p.txt.contains("sign in using"))
    {
        return PageVerdict::Run(SNOWFLAKE_SSO_JS.to_string());
    }
    if is_salesforce(&host) {
        if p.txt.contains("allow") && p.txt.contains("access") && !p.has_password {
            return PageVerdict::Run(SALESFORCE_ALLOW_JS.to_string());
        }
        if p.has_password {
            return if detour_available {
                PageVerdict::Done(Onboard::NeedsMyApps)
            } else {
                PageVerdict::Fail(
                    "Salesforce still asks for its native login after the MyApps sign-in".into(),
                )
            };
        }
    }
    PageVerdict::Continue
}

async fn attempt(
    cfg: &EntraConfig,
    url: &str,
    backend: &str,
    progress: &Progress,
) -> Result<(), ReauthError> {
    let gateway_host = host_of(url);
    let session = BrowserSession::open_default(cfg, true).await?;
    let result = async {
        progress.say(format!(
            "Opening the {backend} onboarding page {}",
            entra_mint::redact_url(url)
        ));
        session.navigate(url).await?;
        let first = entra_mint::drive(cfg, &session, progress, DRIVE_TIMEOUT, |p| {
            sgsc_page_verdict(p, backend, &gateway_host, true)
        })
        .await?;
        if first == Onboard::NeedsMyApps {
            progress.say("Salesforce wants its own login; signing in through MyApps instead…");
            session.navigate(MYAPPS_URL).await?;
            entra_mint::drive(cfg, &session, progress, DRIVE_TIMEOUT, |p| {
                if p.host() == "myapps.microsoft.com" && p.txt.contains("salesforce") {
                    PageVerdict::Done(())
                } else {
                    PageVerdict::Continue
                }
            })
            .await?;
            let opened = session.execute(MYAPPS_TILE_JS).await?;
            if opened.as_str() == Some("no-tile") {
                return Err(ReauthError::Failed(
                    "MyApps shows no Salesforce tile for this account".into(),
                ));
            }
            session.navigate(url).await?;
            entra_mint::drive(cfg, &session, progress, DRIVE_TIMEOUT, |p| {
                sgsc_page_verdict(p, backend, &gateway_host, false)
            })
            .await?;
        }
        progress.say(format!("The gateway reports {backend} connected."));
        Ok(())
    }
    .await;
    session.close().await;
    result
}

pub async fn run(backend: &str, progress: &Progress) -> Result<(), ReauthError> {
    let cfg = EntraConfig::from_env().map_err(|e| ReauthError::NotConfigured(e.to_string()))?;
    let url = onboard_url(&onboard_template()?, backend);
    let mut n = 1;
    loop {
        match attempt(&cfg, &url, backend, progress).await {
            Err(ReauthError::Entra(e)) if entra_mint::should_retry(&e, n) => {
                progress.say(format!("{e}; retrying with a fresh flow"));
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

    const GW: &str = "claude.example.dev";

    fn page(url: &str, txt: &str, has_password: bool) -> Probe {
        Probe {
            url: url.into(),
            txt: txt.into(),
            has_password,
            ..Default::default()
        }
    }

    #[test]
    fn onboard_url_substitutes_the_backend() {
        assert_eq!(
            onboard_url(
                "https://claude.example.dev/_sgsc/auth/onboard/{backend}",
                "dp"
            ),
            "https://claude.example.dev/_sgsc/auth/onboard/dp"
        );
    }

    #[test]
    fn success_requires_the_gateway_to_say_connected() {
        let verdict =
            |url: &str, txt: &str| sgsc_page_verdict(&page(url, txt, false), "dp", GW, true);
        assert!(matches!(
            verdict(
                "https://claude.example.dev/_sgsc/auth/callback/dp?code=x",
                "snowflake connected"
            ),
            PageVerdict::Done(Onboard::Connected)
        ));
        assert!(matches!(
            verdict("https://claude.example.dev/_sgsc/x", "snowflake connected"),
            PageVerdict::Done(Onboard::Connected)
        ));
        // Arriving at the callback is not success until the gateway says so.
        assert!(matches!(
            verdict(
                "https://claude.example.dev/_sgsc/auth/callback/dp?code=x",
                ""
            ),
            PageVerdict::Continue
        ));
        // The prompt that sent us here is not success.
        assert!(matches!(
            verdict(
                "https://claude.example.dev/_sgsc/auth/onboard/dp",
                "connect snowflake to continue"
            ),
            PageVerdict::Continue
        ));
        // Another backend's callback does not count, even when it says
        // connected.
        assert!(matches!(
            verdict("https://claude.example.dev/_sgsc/auth/callback/sf", ""),
            PageVerdict::Continue
        ));
        assert!(matches!(
            verdict(
                "https://claude.example.dev/_sgsc/auth/callback/sf?code=x",
                "salesforce connected"
            ),
            PageVerdict::Continue
        ));
        assert!(matches!(
            verdict(
                "https://claude.example.dev/_sgsc/auth/onboard/sf",
                "salesforce connected"
            ),
            PageVerdict::Continue
        ));
    }

    #[test]
    fn gateway_failures_are_never_success() {
        let v = sgsc_page_verdict(
            &page(
                "https://claude.example.dev/_sgsc/auth/callback/dp?error=access_denied&error_description=user%20cancelled",
                "connected",
                false,
            ),
            "dp",
            GW,
            true,
        );
        match v {
            PageVerdict::Fail(m) => {
                assert_eq!(m, "the gateway reported access_denied: user cancelled")
            }
            _ => panic!("an error callback must fail"),
        }
        let v = sgsc_page_verdict(
            &page(
                "https://claude.example.dev/_sgsc/auth/callback/dp",
                "snowflake not connected",
                false,
            ),
            "dp",
            GW,
            true,
        );
        assert!(matches!(v, PageVerdict::Fail(_)));
        for txt in [
            "snowflake not connected",
            "disconnected",
            "connected? no: failed",
        ] {
            let v = sgsc_page_verdict(
                &page("https://claude.example.dev/_sgsc/status", txt, false),
                "dp",
                GW,
                true,
            );
            assert!(matches!(v, PageVerdict::Continue), "{txt}");
        }
        assert!(says_connected("snowflake connected"));
        assert!(!says_connected("disconnected"));
        assert!(!says_connected("not connected"));
    }

    #[test]
    fn provider_hops_are_handled() {
        let v = sgsc_page_verdict(
            &page(
                "https://acct.snowflakecomputing.com/oauth/authorize",
                "sign in using entra_id_sso",
                false,
            ),
            "dp",
            GW,
            true,
        );
        assert!(matches!(v, PageVerdict::Run(_)));
        let native = page(
            "https://sg.my.salesforce.com/services/oauth2/authorize",
            "username password log in",
            true,
        );
        assert!(matches!(
            sgsc_page_verdict(&native, "sf", GW, true),
            PageVerdict::Done(Onboard::NeedsMyApps)
        ));
        assert!(matches!(
            sgsc_page_verdict(&native, "sf", GW, false),
            PageVerdict::Fail(_)
        ));
        let allow = page(
            "https://sg.my.salesforce.com/setup/secur/RemoteAccessAuthorizationPage.apexp",
            "allow access? allow deny",
            false,
        );
        assert!(matches!(
            sgsc_page_verdict(&allow, "sf", GW, true),
            PageVerdict::Run(_)
        ));
    }

    #[test]
    fn entra_pages_are_left_to_the_driver() {
        let v = sgsc_page_verdict(
            &page(
                "https://login.microsoftonline.com/t/oauth2",
                "connected",
                true,
            ),
            "dp",
            GW,
            true,
        );
        assert!(matches!(v, PageVerdict::Continue));
    }
}
