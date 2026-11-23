//! Reuse the credential-capable Entra driver, capturing only our exact OAuth
//! callback in the remote browser so a loopback redirect needs no open listener.
use std::time::Duration;

use super::ReauthError;
use crate::services::entra_mint::{self, BrowserSession, EntraConfig, EntraError, PageVerdict, Probe, Progress};

fn callback_code(probe: &Probe, redirect: &url::Url, state: &str) -> PageVerdict<Result<String, ReauthError>> {
    let Ok(url) = url::Url::parse(&probe.url) else { return PageVerdict::Continue; };
    if url.origin() != redirect.origin() || url.path() != redirect.path() {
        // Never approve a new grant unattended. Existing browser sessions may
        // satisfy SSO, but new scopes and provider consent need an operator.
        if probe.txt.contains("permissions requested") || probe.txt.contains("review permissions")
            || probe.txt.contains("approve access") || probe.txt.contains("authorize access") {
            return PageVerdict::Done(Err(EntraError::Refused("OAuth consent requires an operator; reconnect this entry in MCP settings".into()).into()));
        }
        return PageVerdict::Continue;
    }
    let params: Vec<_> = url.query_pairs().collect();
    if params.iter().filter(|(k, _)| k == "state").count() != 1
        || !params.iter().any(|(k, v)| k == "state" && v == state) {
        return PageVerdict::Done(Err(ReauthError::Failed("OAuth callback state did not match".into())));
    }
    if let Some((_, error)) = params.iter().find(|(k, _)| k == "error") {
        let human = matches!(error.as_ref(), "consent_required" | "interaction_required" | "access_denied");
        return PageVerdict::Done(Err(if human {
            EntraError::Refused("OAuth provider requires operator interaction or consent".into()).into()
        } else { ReauthError::Failed("OAuth provider did not complete silent sign-in".into()) }));
    }
    if params.iter().filter(|(k, _)| k == "code").count() == 1
        && let Some((_, code)) = params.iter().find(|(k, v)| k == "code" && !v.is_empty()) {
        return PageVerdict::Done(Ok(code.to_string()));
    }
    PageVerdict::Done(Err(ReauthError::Failed("OAuth callback did not contain an authorization code".into())))
}

pub async fn authorize_code(authorize_url: &str, redirect_uri: &str, state: &str, progress: &Progress) -> Result<String, ReauthError> {
    let cfg = EntraConfig::from_env().map_err(|_| ReauthError::NotConfigured("Gateway silent SSO requires the configured vk-entra browser profile".into()))?;
    let redirect = url::Url::parse(redirect_uri).map_err(|_| ReauthError::Failed("Invalid stored OAuth redirect URI".into()))?;
    let session = BrowserSession::open_default(&cfg, true).await?;
    let result = async {
        // Playwright handles this navigation in the browser, never sending the
        // code to a callback listener or logging it as page text.
        let origin = serde_json::to_string(&redirect.origin().ascii_serialization()).unwrap();
        let path = serde_json::to_string(redirect.path()).unwrap();
        session.execute(&format!("await page.route(url => url.origin === {origin} && url.pathname === {path}, route => route.fulfill({{status: 200, contentType: 'text/html', body: '<p>Sign-in completed.</p>'}})); return true;")).await?;
        progress.say("Opening the gateway OAuth sign-in with the persistent browser profile");
        session.navigate(authorize_url).await?;
        entra_mint::drive(&cfg, &session, progress, Duration::from_secs(180), |p| callback_code(p, &redirect, state)).await?
    }.await;
    session.close().await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_requires_exact_origin_path_and_single_state_and_code() {
        let redirect = url::Url::parse("http://localhost:3334/api/mcp-auth/callback").unwrap();
        let verdict = |url: &str| callback_code(&Probe { url: url.into(), ..Default::default() }, &redirect, "expected");
        assert!(matches!(verdict("http://localhost:3334/api/mcp-auth/callback?state=expected&code=secret"), PageVerdict::Done(Ok(_))));
        assert!(matches!(verdict("https://evil.example/api/mcp-auth/callback?state=expected&code=secret"), PageVerdict::Continue));
        assert!(matches!(verdict("http://localhost:3334/api/mcp-auth/callback?state=wrong&code=secret"), PageVerdict::Done(Err(_))));
        assert!(matches!(verdict("http://localhost:3334/api/mcp-auth/callback?state=expected&state=expected&code=secret"), PageVerdict::Done(Err(_))));
        assert!(matches!(verdict("http://localhost:3334/api/mcp-auth/callback?state=expected&error=consent_required"), PageVerdict::Done(Err(ReauthError::Entra(EntraError::Refused(_))))));
    }
}
