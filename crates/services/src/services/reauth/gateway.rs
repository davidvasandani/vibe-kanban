//! Gateway-owned credential repair, supplied by the server to avoid a services
//! dependency on HTTP routing and encrypted gateway storage.
use std::sync::{Arc, OnceLock};

use async_trait::async_trait;

use super::{ReauthAuthState, ReauthError};
use crate::services::entra_mint::Progress;

pub struct GatewayTarget {
    /// Unique friendly name, or UUID when the friendly name is ambiguous.
    pub name: String,
    pub label: String,
    pub auth_state: ReauthAuthState,
    pub auth_message: Option<String>,
    pub renewal_due: bool,
}

#[async_trait]
pub trait GatewayReauthProvider: Send + Sync {
    async fn targets(&self) -> Result<Vec<GatewayTarget>, ReauthError>;
    /// Resolve names and UUID aliases against active, locally bound inventory.
    async fn canonicalize(&self, name: &str) -> Result<String, ReauthError>;
    async fn renew(&self, name: &str, progress: &Progress) -> Result<(), ReauthError>;
    async fn verify(&self, name: &str) -> Result<(), String>;
}

static PROVIDER: OnceLock<Arc<dyn GatewayReauthProvider>> = OnceLock::new();

pub fn register(provider: Arc<dyn GatewayReauthProvider>) {
    assert!(
        PROVIDER.set(provider).is_ok(),
        "gateway reauth provider registered twice"
    );
}

pub fn provider() -> Option<&'static Arc<dyn GatewayReauthProvider>> {
    PROVIDER.get()
}

pub(super) fn required() -> Result<&'static Arc<dyn GatewayReauthProvider>, ReauthError> {
    provider().ok_or_else(|| {
        ReauthError::NotConfigured("MCP gateway is not configured on this host".into())
    })
}

/// Refresh grants need no Entra factors and remain usable after an unrelated
/// account refusal. Only browser fallback takes the shared account gate.
pub async fn authorize_code(
    name: &str,
    authorize_url: &str,
    redirect_uri: &str,
    state: &str,
    progress: &Progress,
) -> Result<String, ReauthError> {
    use super::{ReauthTargetId, ReauthTrigger};
    use crate::services::entra_mint::EntraError;
    let id = ReauthTargetId::McpGateway(name.to_owned());
    let (trigger, requested_at) = super::with_registry(|states| {
        states
            .get(&id)
            .and_then(|s| s.last_run.as_ref())
            .map(|r| (r.trigger, r.started_at))
    })
    .unwrap_or((ReauthTrigger::Agent, chrono::Utc::now()));
    let _turn = super::entra_run_lock().lock().await;
    if let Some(reason) = super::entra_gate(trigger, super::entra_refusal().as_ref(), requested_at)
    {
        return Err(ReauthError::Entra(EntraError::Refused(reason)));
    }
    let result =
        super::gateway_browser::authorize_code(authorize_url, redirect_uri, state, progress)
            .await
            .map_err(super::normalize_refusal);
    match &result {
        Err(ReauthError::Entra(EntraError::Refused(reason))) => {
            super::set_entra_refusal(Some(reason.clone()))
        }
        Ok(_) => super::set_entra_refusal(None),
        _ => {}
    }
    result
}
