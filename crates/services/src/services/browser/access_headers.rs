//! Cloudflare Access service-token headers for the managed browser.
//!
//! When the deployment provides a service token, the CDP driver attaches
//! `CF-Access-Client-Id` / `CF-Access-Client-Secret` to https requests whose
//! host is one of the configured domains (or a subdomain of one), so agents
//! can view Access-gated apps without an interactive login. Scope fails
//! closed: without at least one domain the headers are sent nowhere.

use std::fmt;

use serde_json::{Map, Value, json};
use url::{Host, Url};

pub const CLIENT_ID_ENV: &str = "VIBE_BROWSER_CF_ACCESS_CLIENT_ID";
pub const CLIENT_SECRET_ENV: &str = "VIBE_BROWSER_CF_ACCESS_CLIENT_SECRET";
pub const DOMAINS_ENV: &str = "VIBE_BROWSER_CF_ACCESS_DOMAINS";

const CLIENT_ID_HEADER: &str = "CF-Access-Client-Id";
const CLIENT_SECRET_HEADER: &str = "CF-Access-Client-Secret";

#[derive(Clone)]
pub struct AccessHeaders {
    client_id: String,
    client_secret: String,
    domains: Vec<String>,
}

impl fmt::Debug for AccessHeaders {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccessHeaders")
            .field("client_id", &self.client_id)
            .field("client_secret", &"<redacted>")
            .field("domains", &self.domains)
            .finish()
    }
}

impl AccessHeaders {
    /// Build from raw values. `None` unless the id, the secret and at least
    /// one valid domain are all present.
    pub fn from_values(client_id: &str, client_secret: &str, domains_csv: &str) -> Option<Self> {
        let client_id = client_id.trim();
        let client_secret = client_secret.trim();
        if client_id.is_empty() || client_secret.is_empty() {
            return None;
        }
        let mut domains: Vec<String> = domains_csv
            .split(',')
            .map(|d| d.trim().trim_matches('.').to_ascii_lowercase())
            .filter(|d| {
                !d.is_empty()
                    && d.contains('.')
                    && d.chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
            })
            .collect();
        domains.dedup();
        if domains.is_empty() {
            return None;
        }
        Some(Self {
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            domains,
        })
    }

    pub fn from_env() -> Option<Self> {
        let var = |name| std::env::var(name).unwrap_or_default();
        Self::from_values(
            &var(CLIENT_ID_ENV),
            &var(CLIENT_SECRET_ENV),
            &var(DOMAINS_ENV),
        )
    }

    pub fn domains(&self) -> &[String] {
        &self.domains
    }

    /// True for https URLs whose host equals a configured domain or is a
    /// subdomain of one (label boundary). IP hosts never match.
    pub fn applies_to(&self, url: &str) -> bool {
        let Ok(parsed) = Url::parse(url) else {
            return false;
        };
        if parsed.scheme() != "https" {
            return false;
        }
        let Some(Host::Domain(host)) = parsed.host() else {
            return false;
        };
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        self.domains.iter().any(|domain| {
            host == *domain
                || host
                    .strip_suffix(domain.as_str())
                    .is_some_and(|prefix| prefix.ends_with('.'))
        })
    }

    /// `Fetch.enable` patterns that pause every https URL mentioning a
    /// configured domain. CDP wildcards span any characters, so this also
    /// catches explicit ports and trailing-dot hosts (and some out-of-scope
    /// URLs); `applies_to` decides per request whether headers are added.
    pub fn fetch_patterns(&self) -> Vec<Value> {
        self.domains
            .iter()
            .map(|d| json!({ "urlPattern": format!("https://*{d}*"), "requestStage": "Request" }))
            .collect()
    }

    /// Header entries for `Fetch.continueRequest`: the request's existing
    /// headers minus any CF-Access client headers, plus ours.
    pub fn merge_headers(&self, existing: Option<&Map<String, Value>>) -> Vec<Value> {
        let mut headers: Vec<Value> = existing
            .into_iter()
            .flatten()
            .filter(|(name, _)| {
                !name.eq_ignore_ascii_case(CLIENT_ID_HEADER)
                    && !name.eq_ignore_ascii_case(CLIENT_SECRET_HEADER)
            })
            .filter_map(|(name, value)| {
                value
                    .as_str()
                    .map(|value| json!({ "name": name, "value": value }))
            })
            .collect();
        headers.push(json!({ "name": CLIENT_ID_HEADER, "value": self.client_id }));
        headers.push(json!({ "name": CLIENT_SECRET_HEADER, "value": self.client_secret }));
        headers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(domains: &str) -> AccessHeaders {
        AccessHeaders::from_values("id.access", "s3cret", domains).expect("configured")
    }

    #[test]
    fn requires_id_secret_and_a_domain() {
        assert!(AccessHeaders::from_values("", "s", "example.com").is_none());
        assert!(AccessHeaders::from_values("i", " ", "example.com").is_none());
        assert!(AccessHeaders::from_values("i", "s", "").is_none());
        assert!(AccessHeaders::from_values("i", "s", " , ,").is_none());
        assert!(AccessHeaders::from_values("i", "s", "localhost").is_none());
        assert!(AccessHeaders::from_values("i", "s", "*.example.com").is_none());
    }

    #[test]
    fn normalizes_domains() {
        let h = headers(" .Example.COM. , other.dev");
        assert_eq!(h.domains(), ["example.com", "other.dev"]);
    }

    #[test]
    fn matches_exact_and_subdomains_on_label_boundary() {
        let h = headers("example.com");
        assert!(h.applies_to("https://example.com/"));
        assert!(h.applies_to("https://app.example.com/path?q=1"));
        assert!(h.applies_to("https://a.b.EXAMPLE.com:8443/"));
        assert!(h.applies_to("https://app.example.com./"));
        assert!(!h.applies_to("https://evilexample.com/"));
        assert!(!h.applies_to("https://example.com.attacker.net/"));
        assert!(!h.applies_to("https://attacker.net/?u=https://example.com"));
    }

    #[test]
    fn rejects_non_https_ips_and_garbage() {
        let h = headers("example.com");
        assert!(!h.applies_to("http://app.example.com/"));
        assert!(!h.applies_to("wss://app.example.com/"));
        assert!(!h.applies_to("https://93.184.216.34/"));
        assert!(!h.applies_to("https://[::1]/"));
        assert!(!h.applies_to("not a url"));
        assert!(!h.applies_to("about:blank"));
    }

    #[test]
    fn userinfo_does_not_confuse_host() {
        let h = headers("example.com");
        assert!(!h.applies_to("https://example.com@attacker.net/"));
        assert!(h.applies_to("https://attacker.net@app.example.com/"));
    }

    #[test]
    fn merge_replaces_existing_cf_headers_and_keeps_others() {
        let h = headers("example.com");
        let existing = json!({
            "Accept": "text/html",
            "cf-access-client-id": "spoofed",
            "CF-ACCESS-CLIENT-SECRET": "spoofed",
        });
        let merged = h.merge_headers(existing.as_object());
        assert_eq!(merged.len(), 3);
        assert!(merged.contains(&json!({ "name": "Accept", "value": "text/html" })));
        assert!(merged.contains(&json!({ "name": "CF-Access-Client-Id", "value": "id.access" })));
        assert!(merged.contains(&json!({ "name": "CF-Access-Client-Secret", "value": "s3cret" })));
        assert_eq!(h.merge_headers(None).len(), 2);
    }

    /// CDP `urlPattern` semantics: `*` matches any run of characters.
    fn cdp_pattern_matches(pattern: &str, url: &str) -> bool {
        let parts: Vec<&str> = pattern.split('*').collect();
        let mut rest = url;
        for (i, part) in parts.iter().enumerate() {
            if i == 0 {
                let Some(r) = rest.strip_prefix(part) else {
                    return false;
                };
                rest = r;
            } else if i == parts.len() - 1 {
                return rest.ends_with(part);
            } else {
                let Some(idx) = rest.find(part) else {
                    return false;
                };
                rest = &rest[idx + part.len()..];
            }
        }
        rest.is_empty()
    }

    #[test]
    fn fetch_patterns_pause_every_in_scope_url() {
        let h = headers("example.com");
        let patterns: Vec<String> = h
            .fetch_patterns()
            .iter()
            .map(|p| p["urlPattern"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(patterns, ["https://*example.com*"]);
        for url in [
            "https://example.com/",
            "https://app.example.com/path?q=1",
            "https://app.example.com:8443/",
            "https://app.example.com./",
            "https://attacker.net@app.example.com/",
        ] {
            assert!(h.applies_to(url), "{url}");
            assert!(
                patterns.iter().any(|p| cdp_pattern_matches(p, url)),
                "{url} would not pause"
            );
        }
    }

    #[test]
    fn debug_redacts_secret() {
        let rendered = format!("{:?}", headers("example.com"));
        assert!(!rendered.contains("s3cret"));
        assert!(rendered.contains("<redacted>"));
    }
}
