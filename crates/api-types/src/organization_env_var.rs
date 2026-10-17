use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// Prefix of a 1Password secret reference (`op://vault/item/field`).
pub const SECRET_REFERENCE_PREFIX: &str = "op://";

/// Quote pairs 1Password (and smart-quote substitution) wrap around a copied
/// secret reference.
const REFERENCE_QUOTES: [(char, char); 4] = [('"', '"'), ('\'', '\''), ('“', '”'), ('‘', '’')];

/// Return the bare 1Password reference when the whole `value` is one,
/// optionally surrounded by whitespace and one matching pair of quotes (as
/// 1Password's "Copy Secret Reference" produces). Returns `None` for every
/// other value; callers must then use the original value byte-for-byte, since
/// quotes and whitespace may be part of a literal secret.
pub fn normalize_secret_reference(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let unquoted = REFERENCE_QUOTES
        .iter()
        .find_map(|(open, close)| {
            trimmed
                .strip_prefix(*open)
                .and_then(|rest| rest.strip_suffix(*close))
        })
        .map(str::trim)
        .unwrap_or(trimmed);
    unquoted
        .starts_with(SECRET_REFERENCE_PREFIX)
        .then(|| unquoted.to_owned())
}

/// Organization-scoped environment variable. Literal values are never returned
/// in listing responses to avoid exposing secrets; clients overwrite by
/// PATCHing a new value. A value that is a 1Password reference is a pointer,
/// not a secret, and is returned as `reference` so it can be displayed.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct OrganizationEnvVar {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListOrganizationEnvVarsResponse {
    pub env_vars: Vec<OrganizationEnvVar>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateOrganizationEnvVarRequest {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateOrganizationEnvVarResponse {
    pub env_var: OrganizationEnvVar,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpdateOrganizationEnvVarRequest {
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpdateOrganizationEnvVarResponse {
    pub env_var: OrganizationEnvVar,
}

/// A resolved (decrypted) organization env var. Unlike the listing types above,
/// this carries the plaintext `value` and is only returned to callers with
/// access to the owning organization's project, for injection into agent
/// processes started against that project.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ResolvedEnvVar {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ResolvedOrganizationEnvVarsResponse {
    pub env_vars: Vec<ResolvedEnvVar>,
}

#[cfg(test)]
mod tests {
    use super::normalize_secret_reference;

    #[test]
    fn bare_and_padded_references_normalize() {
        for input in [
            "op://Homelab/alderbridge nix PAT/credential",
            "  op://Homelab/alderbridge nix PAT/credential\n",
            "\"op://Homelab/alderbridge nix PAT/credential\"",
            " \" op://Homelab/alderbridge nix PAT/credential \" ",
            "'op://Homelab/alderbridge nix PAT/credential'",
            "“op://Homelab/alderbridge nix PAT/credential”",
            "‘op://Homelab/alderbridge nix PAT/credential’",
        ] {
            assert_eq!(
                normalize_secret_reference(input).as_deref(),
                Some("op://Homelab/alderbridge nix PAT/credential"),
                "{input:?}"
            );
        }
    }

    #[test]
    fn incomplete_reference_is_still_a_reference() {
        assert_eq!(normalize_secret_reference("\"op://").as_deref(), None);
        assert_eq!(
            normalize_secret_reference("\"op://\"").as_deref(),
            Some("op://")
        );
    }

    #[test]
    fn literals_and_near_misses_are_not_references() {
        for input in [
            "",
            "\"\"",
            "plain-secret",
            "\"quoted literal\"",
            "\"op://x'",
            "'op://x\"",
            "\"\"op://x\"\"",
            "OP://vault/item/field",
            "prefix op://vault/item/field",
        ] {
            assert_eq!(normalize_secret_reference(input), None, "{input:?}");
        }
    }
}
