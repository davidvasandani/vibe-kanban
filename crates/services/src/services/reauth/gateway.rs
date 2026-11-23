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
