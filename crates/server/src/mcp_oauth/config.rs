use url::{Host, Url};

/// Names the public origin that remote OAuth clients reach. Unset (the
/// default) keeps every OAuth route at 404, so local installs gain no surface.
pub const PUBLIC_URL_ENV: &str = "VK_MCP_OAUTH_PUBLIC_URL";

/// Path of the OAuth-protected MCP resource beneath the issuer origin.
pub const RESOURCE_PATH: &str = "/oauth/mcp";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpOAuthConfig {
    /// Origin without a trailing slash, e.g. `https://vibe.vasandani.dev`.
    pub issuer: String,
}

impl McpOAuthConfig {
    pub fn from_env() -> Option<Self> {
        let raw = std::env::var(PUBLIC_URL_ENV).ok()?;
        if raw.trim().is_empty() {
            return None;
        }
        match Self::parse(&raw) {
            Ok(config) => Some(config),
            Err(reason) => {
                tracing::warn!("{PUBLIC_URL_ENV} ignored ({reason}); MCP OAuth stays disabled");
                None
            }
        }
    }

    /// Accept an `https` origin, or `http` on a loopback host for development.
    /// The issuer must be a bare origin so the RFC 8414/9728 well-known paths
    /// live at the host root.
    pub fn parse(raw: &str) -> Result<Self, &'static str> {
        let url = Url::parse(raw.trim()).map_err(|_| "not an absolute URL")?;
        match url.scheme() {
            "https" => {}
            "http" if is_loopback(&url) => {}
            _ => return Err("must be https (or http on a loopback host)"),
        }
        if url.host().is_none() {
            return Err("missing host");
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err("must not contain credentials");
        }
        if url.query().is_some() || url.fragment().is_some() {
            return Err("must not contain a query or fragment");
        }
        if !matches!(url.path(), "" | "/") {
            return Err("must be an origin without a path");
        }
        let issuer = url.as_str().trim_end_matches('/').to_string();
        Ok(Self { issuer })
    }

    pub fn resource(&self) -> String {
        format!("{}{RESOURCE_PATH}", self.issuer)
    }

    pub fn resource_metadata_url(&self) -> String {
        format!(
            "{}/.well-known/oauth-protected-resource{RESOURCE_PATH}",
            self.issuer
        )
    }

    pub fn endpoint(&self, path: &str) -> String {
        format!("{}{path}", self.issuer)
    }
}

pub(crate) fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_https_origin_and_trims_slash() {
        let config = McpOAuthConfig::parse("https://vibe.vasandani.dev/").unwrap();
        assert_eq!(config.issuer, "https://vibe.vasandani.dev");
        assert_eq!(config.resource(), "https://vibe.vasandani.dev/oauth/mcp");
        assert_eq!(
            config.resource_metadata_url(),
            "https://vibe.vasandani.dev/.well-known/oauth-protected-resource/oauth/mcp"
        );
    }

    #[test]
    fn accepts_loopback_http_only() {
        assert!(McpOAuthConfig::parse("http://localhost:3000").is_ok());
        assert!(McpOAuthConfig::parse("http://127.0.0.1:3000").is_ok());
        assert!(McpOAuthConfig::parse("http://vibe.vasandani.dev").is_err());
    }

    #[test]
    fn rejects_paths_queries_and_credentials() {
        assert!(McpOAuthConfig::parse("https://h/sub").is_err());
        assert!(McpOAuthConfig::parse("https://h/?a=b").is_err());
        assert!(McpOAuthConfig::parse("https://u:p@h").is_err());
        assert!(McpOAuthConfig::parse("ftp://h").is_err());
        assert!(McpOAuthConfig::parse("not a url").is_err());
    }
}
