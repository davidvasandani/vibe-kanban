use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

const NAME_MAX_LEN: usize = 256;
const EMAIL_MAX_LEN: usize = 320;
const ALLOWED_EMAILS_MAX: usize = 50;

/// The git identity an organization's workspaces commit as. Agent hosts write
/// `name`/`email` into each workspace worktree's git config and install a
/// pre-commit check that rejects any author or committer email other than
/// `email` or one of `allowed_emails` (agent and bot identities).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct OrganizationCommitIdentity {
    pub organization_id: Uuid,
    pub name: String,
    pub email: String,
    pub allowed_emails: Vec<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct GetOrganizationCommitIdentityResponse {
    pub commit_identity: Option<OrganizationCommitIdentity>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpsertOrganizationCommitIdentityRequest {
    pub name: String,
    pub email: String,
    #[serde(default)]
    pub allowed_emails: Vec<String>,
}

/// A validated identity, ready to store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedCommitIdentity {
    pub name: String,
    pub email: String,
    pub allowed_emails: Vec<String>,
}

/// Validate and normalize an identity: trims every field, lowercases emails,
/// and drops blank or duplicate allowlist entries (including `email` itself).
pub fn normalize_commit_identity(
    request: &UpsertOrganizationCommitIdentityRequest,
) -> Result<NormalizedCommitIdentity, String> {
    let name = request.name.trim();
    if name.is_empty() {
        return Err("Commit name must not be empty".to_owned());
    }
    if name.len() > NAME_MAX_LEN {
        return Err("Commit name is too long".to_owned());
    }
    if name.contains(['<', '>', '\n', '\r']) {
        return Err("Commit name must not contain '<', '>' or line breaks".to_owned());
    }

    let email = normalize_email(&request.email)?;

    let mut allowed_emails: Vec<String> = Vec::new();
    for raw in &request.allowed_emails {
        if raw.trim().is_empty() {
            continue;
        }
        let allowed = normalize_email(raw)?;
        if allowed != email && !allowed_emails.contains(&allowed) {
            allowed_emails.push(allowed);
        }
    }
    if allowed_emails.len() > ALLOWED_EMAILS_MAX {
        return Err(format!(
            "At most {ALLOWED_EMAILS_MAX} allowed emails are supported"
        ));
    }

    Ok(NormalizedCommitIdentity {
        name: name.to_owned(),
        email,
        allowed_emails,
    })
}

/// The pre-commit check matches addresses case-insensitively, so they are
/// stored lowercased.
fn normalize_email(raw: &str) -> Result<String, String> {
    let email = raw.trim().to_ascii_lowercase();
    let invalid = || format!("'{}' is not a valid email address", raw.trim());
    if email.len() > EMAIL_MAX_LEN {
        return Err(invalid());
    }
    let Some((local, domain)) = email.split_once('@') else {
        return Err(invalid());
    };
    // Git writes the address between '<' and '>' in the commit header, so
    // those, whitespace, and a second '@' can never be part of a usable one.
    let bad_char = |c: char| c.is_whitespace() || matches!(c, '<' | '>' | '@' | ',' | '"');
    if local.is_empty() || domain.is_empty() || email.chars().filter(|&c| c == '@').count() != 1 {
        return Err(invalid());
    }
    if local.chars().any(bad_char) || domain.chars().any(bad_char) {
        return Err(invalid());
    }
    Ok(email)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(
        name: &str,
        email: &str,
        allowed: &[&str],
    ) -> UpsertOrganizationCommitIdentityRequest {
        UpsertOrganizationCommitIdentityRequest {
            name: name.to_owned(),
            email: email.to_owned(),
            allowed_emails: allowed.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    #[test]
    fn normalizes_and_dedupes() {
        let identity = normalize_commit_identity(&request(
            "  David Vasandani ",
            " David@Vasandani.ME ",
            &[
                "noreply@anthropic.com",
                "",
                "NoReply@Anthropic.com",
                "david@vasandani.me",
                "  noreply@vibekanban.com ",
            ],
        ))
        .unwrap();
        assert_eq!(identity.name, "David Vasandani");
        assert_eq!(identity.email, "david@vasandani.me");
        assert_eq!(
            identity.allowed_emails,
            vec!["noreply@anthropic.com", "noreply@vibekanban.com"]
        );
    }

    #[test]
    fn rejects_bad_names() {
        assert!(normalize_commit_identity(&request(" ", "a@b.c", &[])).is_err());
        assert!(normalize_commit_identity(&request("A <x>", "a@b.c", &[])).is_err());
        assert!(normalize_commit_identity(&request("A\nB", "a@b.c", &[])).is_err());
    }

    #[test]
    fn rejects_bad_emails() {
        for email in [
            "", "nobody", "@b.c", "a@", "a@@b.c", "a b@c.d", "a@b>c", "a@b,c",
        ] {
            assert!(
                normalize_commit_identity(&request("A", email, &[])).is_err(),
                "{email:?} should be rejected"
            );
        }
        assert!(normalize_commit_identity(&request("A", "a@b.c", &["nope"])).is_err());
    }

    #[test]
    fn accepts_hostname_and_noreply_style_addresses() {
        for email in [
            "noreply@github.com",
            "547876+davidvasandani@users.noreply.github.com",
            "29139614+renovate[bot]@users.noreply.github.com",
        ] {
            assert!(normalize_commit_identity(&request("A", "a@b.c", &[email])).is_ok());
        }
    }
}
