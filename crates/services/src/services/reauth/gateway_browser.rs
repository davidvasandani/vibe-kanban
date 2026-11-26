//! Reuse the credential-capable Entra driver, capturing only our exact OAuth
//! callback in the remote browser so a loopback redirect needs no open listener.
use super::ReauthError;
use crate::services::entra_mint::{
    self, BrowserSession, EntraConfig, EntraError, PageVerdict, Probe, Progress,
};

fn callback_code(
    probe: &Probe,
    redirect: &url::Url,
    state: &str,
) -> PageVerdict<Result<String, ReauthError>> {
    let Ok(url) = url::Url::parse(&probe.url) else {
        return PageVerdict::Continue;
    };
    if url.origin() != redirect.origin() || url.path() != redirect.path() {
        // Never approve a new grant unattended. Existing browser sessions may
        // satisfy SSO, but new scopes and provider consent need an operator.
        if probe.txt.contains("permissions requested")
            || probe.txt.contains("review permissions")
            || probe.txt.contains("approve access")
            || probe.txt.contains("authorize access")
        {
            return PageVerdict::Done(Err(ReauthError::Refused(
                "OAuth consent requires an operator; reconnect this entry in MCP settings".into(),
            )));
        }
        return PageVerdict::Continue;
    }
    let params: Vec<_> = url.query_pairs().collect();
    if params.iter().filter(|(k, _)| k == "state").count() != 1
        || !params.iter().any(|(k, v)| k == "state" && v == state)
    {
        return PageVerdict::Done(Err(ReauthError::Failed(
            "OAuth callback state did not match".into(),
        )));
    }
    if let Some((_, error)) = params.iter().find(|(k, _)| k == "error") {
        if let Some(reason) = params
            .iter()
            .find(|(k, _)| k == "error_description")
            .and_then(|(_, description)| entra_mint::refusal_in(description))
        {
            return PageVerdict::Done(Err(EntraError::Refused(reason.into()).into()));
        }
        let human = matches!(
            error.as_ref(),
            "consent_required" | "interaction_required" | "access_denied"
        );
        return PageVerdict::Done(Err(if human {
            ReauthError::Refused("OAuth provider requires operator interaction or consent".into())
        } else {
            ReauthError::Failed("OAuth provider did not complete silent sign-in".into())
        }));
    }
    if params.iter().filter(|(k, _)| k == "code").count() == 1
        && let Some((_, code)) = params.iter().find(|(k, v)| k == "code" && !v.is_empty())
    {
        return PageVerdict::Done(Ok(code.to_string()));
    }
    PageVerdict::Done(Err(ReauthError::Failed(
        "OAuth callback did not contain an authorization code".into(),
    )))
}

/// Routes registered by the browser service exist for ONE execute call only.
/// Keep interception, navigation, and the wait together; returning after route()
/// and navigating in another call loses the authorization code on loopback.
fn silent_script(authorize_url: &str, redirect: &url::Url, email: &str) -> String {
    let origin = serde_json::to_string(&redirect.origin().ascii_serialization()).unwrap();
    let path = serde_json::to_string(redirect.path()).unwrap();
    let authorize = serde_json::to_string(authorize_url).unwrap();
    let email = serde_json::to_string(email).unwrap();
    format!(
        r#"
const matches = url => url.origin === {origin} && url.pathname === {path};
let callback = null;
// Playwright only routes the first URL in an HTTP redirect chain. Observe
// requests too so a redirect to loopback is captured even if no listener is
// running there and goto rejects with ERR_CONNECTION_REFUSED.
const observe = request => {{
    const url = new URL(request.url());
    if (matches(url)) callback = request.url();
}};
page.on('request', observe);
const handler = async route => {{
    callback = route.request().url();
    await route.fulfill({{status: 200, contentType: 'text/html', body: '<p>Sign-in completed.</p>'}});
}};
await page.route(matches, handler);
try {{
    const deadline = Date.now() + 25000;
    try {{ await page.goto({authorize}, {{waitUntil: 'domcontentloaded', timeout: 15000}}); }}
    catch (_) {{ if (!callback) return {{kind: 'transport'}}; }}
    let selectedAccount = false;
    let lastEntraText = '';
    while (Date.now() < deadline) {{
        if (callback) return {{kind: 'callback', url: callback}};
        let txt = '';
        try {{ txt = (await page.locator('body').innerText({{timeout: 2000}})).toLowerCase(); }} catch (_) {{}}
        const url = new URL(page.url());
        const entra = url.protocol === 'https:' && url.hostname === 'login.microsoftonline.com';
        if (entra) lastEntraText = txt.slice(0, 16000);
        if (/permissions requested|review permissions|approve access|authorize access/.test(txt))
            return {{kind: 'human', entra_text: lastEntraText}};
        if (entra && !selectedAccount) {{
            const account = page.getByText({email}, {{exact: true}}).first();
            if (await account.isVisible().catch(() => false)) {{
                selectedAccount = true;
                await account.click({{timeout: 5000}});
                continue;
            }}
        }}
        if (entra && txt.includes('stay signed in')) {{
            await page.locator('#idSIButton9').click({{timeout: 5000}});
            continue;
        }}
        const password = await page.locator('input[type="password"]').first().isVisible().catch(() => false);
        const challenge = await page.locator('input[name="loginfmt"], input[name="otc"]').first().isVisible().catch(() => false);
        if (password || challenge) return {{kind: entra ? 'entra' : 'human', entra_text: lastEntraText}};
        if (entra && /approve sign in|approve the request|authentication app|security key/.test(txt))
            return {{kind: 'entra', entra_text: lastEntraText}};
        await page.waitForTimeout(250);
    }}
    return {{kind: 'timeout', entra_text: lastEntraText}};
}} finally {{
    page.off('request', observe);
    await page.unroute(matches, handler);
}}
"#
    )
}

pub async fn authorize_code(
    authorize_url: &str,
    redirect_uri: &str,
    state: &str,
    progress: &Progress,
) -> Result<String, ReauthError> {
    let cfg = EntraConfig::from_env().map_err(|_| {
        ReauthError::NotConfigured(
            "Gateway silent SSO requires the configured vk-entra browser profile".into(),
        )
    })?;
    let redirect = url::Url::parse(redirect_uri)
        .map_err(|_| ReauthError::Failed("Invalid stored OAuth redirect URI".into()))?;
    let script = silent_script(authorize_url, &redirect, &cfg.email);
    for warmed in [false, true] {
        progress.say("Trying gateway OAuth sign-in with the persistent browser profile");
        let session = BrowserSession::open_default(&cfg, true).await?;
        let result = session.execute(&script).await;
        session.close().await;
        let result = result?;
        let kind = result.get("kind").and_then(serde_json::Value::as_str);
        if let Some(reason) = result
            .get("entra_text")
            .and_then(serde_json::Value::as_str)
            .and_then(entra_mint::refusal_in)
        {
            // Only text observed on the HTTPS Entra origin enters this path.
            // Preserve the account-wide gate without echoing provider text.
            return Err(EntraError::Refused(reason.into()).into());
        }
        let needs_entra = match kind {
            Some("callback") => {
                let callback = result.get("url").and_then(serde_json::Value::as_str)
                    .ok_or_else(|| ReauthError::Failed("OAuth callback was absent".into()))?;
                let url = url::Url::parse(callback).map_err(|_| ReauthError::Failed("OAuth callback was invalid".into()))?;
                // Silent OAuth may explicitly ask us to refresh the shared SSO
                // session. Never treat a state mismatch as permission to do so.
                let validated = callback_code(&Probe { url: callback.into(), ..Default::default() }, &redirect, state);
                if let PageVerdict::Done(Err(ReauthError::Entra(EntraError::Refused(reason)))) = &validated {
                    return Err(EntraError::Refused(reason.clone()).into());
                }
                let matching = url.origin() == redirect.origin() && url.path() == redirect.path()
                    && url.query_pairs().filter(|(k, _)| k == "state").count() == 1
                    && url.query_pairs().any(|(k, v)| k == "state" && v == state);
                if matching && url.query_pairs().any(|(k, v)| k == "error" && matches!(v.as_ref(), "login_required" | "interaction_required")) {
                    true
                } else {
                    return match validated {
                        PageVerdict::Done(result) => result,
                        _ => Err(ReauthError::Failed("OAuth callback did not match this flow".into())),
                    };
                }
            }
            Some("entra") => true,
            Some("human") => return Err(ReauthError::Refused("OAuth provider requires interactive sign-in or consent; reconnect this entry in MCP settings".into())),
            Some("timeout") => return Err(EntraError::FlowTimeout.into()),
            _ => return Err(ReauthError::Failed("Gateway OAuth page could not be reached".into())),
        };
        if needs_entra {
            if warmed {
                return Err(ReauthError::Refused("The provider still requires interaction after refreshing the Entra session; reconnect in MCP settings".into()));
            }
            // Close the gateway session BEFORE this call: both use vk-entra's
            // profile lock. The established driver handles password/TOTP and
            // surfaces account-wide refusals without a second factors engine.
            entra_mint::refresh_session(&cfg, progress).await?;
        }
    }
    Err(ReauthError::Failed("Gateway OAuth did not finish".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_requires_exact_origin_path_and_single_state_and_code() {
        let redirect = url::Url::parse("http://localhost:3334/api/mcp-auth/callback").unwrap();
        let verdict = |url: &str| {
            callback_code(
                &Probe {
                    url: url.into(),
                    ..Default::default()
                },
                &redirect,
                "expected",
            )
        };
        assert!(matches!(
            verdict("http://localhost:3334/api/mcp-auth/callback?state=expected&code=secret"),
            PageVerdict::Done(Ok(_))
        ));
        assert!(matches!(
            verdict("https://evil.example/api/mcp-auth/callback?state=expected&code=secret"),
            PageVerdict::Continue
        ));
        assert!(matches!(
            verdict("http://localhost:3334/api/mcp-auth/callback?state=wrong&code=secret"),
            PageVerdict::Done(Err(_))
        ));
        assert!(matches!(
            verdict(
                "http://localhost:3334/api/mcp-auth/callback?state=expected&state=expected&code=secret"
            ),
            PageVerdict::Done(Err(_))
        ));
        assert!(matches!(
            verdict(
                "http://localhost:3334/api/mcp-auth/callback?state=expected&error=consent_required"
            ),
            PageVerdict::Done(Err(ReauthError::Refused(_)))
        ));
    }

    #[test]
    fn definitive_callback_refusals_are_account_wide_and_state_checked() {
        let redirect = url::Url::parse("http://localhost:3334/api/mcp-auth/callback").unwrap();
        for code in ["AADSTS50053", "AADSTS53003"] {
            let probe = Probe {
                url: format!(
                    "{redirect}?state=expected&error=interaction_required&error_description={code}"
                ),
                ..Default::default()
            };
            assert!(matches!(
                callback_code(&probe, &redirect, "expected"),
                PageVerdict::Done(Err(ReauthError::Entra(EntraError::Refused(_))))
            ));
            assert!(matches!(
                callback_code(&probe, &redirect, "wrong"),
                PageVerdict::Done(Err(ReauthError::Failed(_)))
            ));
        }
    }
}
