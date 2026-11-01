//! Which pull requests `merge_pr admin=true` may merge past GitHub's
//! `blocked` verdict.
//!
//! The allowlist comes only from the deployment environment
//! (`VK_ADMIN_MERGE_ALLOW`), never from Settings: agents can write VK's
//! config through its API, and a bypass they could grant themselves would be
//! no control at all. Unset means no admin merges.

use std::collections::{HashMap, HashSet};

pub const ADMIN_MERGE_ALLOW: &str = "VK_ADMIN_MERGE_ALLOW";

/// Repositories (`owner/repo`, lower-cased) mapped to the PR authors whose
/// PRs may be admin-merged there.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AdminMergePolicy {
    rules: HashMap<String, HashSet<String>>,
}

impl AdminMergePolicy {
    pub fn from_env() -> Self {
        Self::parse(&std::env::var(ADMIN_MERGE_ALLOW).unwrap_or_default())
    }

    /// Entries are `owner/repo:login[,login…]`, separated by `;` or
    /// whitespace. Matching is case-insensitive and there are no wildcards;
    /// malformed entries are logged and ignored, never widened.
    pub fn parse(raw: &str) -> Self {
        let mut rules: HashMap<String, HashSet<String>> = HashMap::new();
        for entry in raw
            .split(|c: char| c == ';' || c.is_whitespace())
            .filter(|entry| !entry.is_empty())
        {
            match parse_entry(entry) {
                Some((repo, authors)) => rules.entry(repo).or_default().extend(authors),
                None => tracing::warn!(
                    entry,
                    "Ignoring malformed {ADMIN_MERGE_ALLOW} entry (expected owner/repo:login[,login])"
                ),
            }
        }
        Self { rules }
    }

    /// `Ok` when `repo` (`owner/repo`) lists `author`; otherwise the reason.
    pub fn allows(&self, repo: Option<&str>, author: Option<&str>) -> Result<(), String> {
        let repo = repo.ok_or("GitHub did not report the PR's base repository")?;
        let author = author.ok_or("GitHub did not report the PR's author")?;
        let Some(authors) = self.rules.get(&repo.to_ascii_lowercase()) else {
            return Err(if self.rules.is_empty() {
                format!(
                    "admin merges are disabled on this Vibe Kanban host ({ADMIN_MERGE_ALLOW} is unset)"
                )
            } else {
                format!("{repo} is not in {ADMIN_MERGE_ALLOW}")
            });
        };
        if authors.contains(&author.to_ascii_lowercase()) {
            Ok(())
        } else {
            Err(format!(
                "PRs by {author} are not admin-mergeable in {repo} ({ADMIN_MERGE_ALLOW})"
            ))
        }
    }
}

fn parse_entry(entry: &str) -> Option<(String, Vec<String>)> {
    let (repo, authors) = entry.split_once(':')?;
    let (owner, name) = repo.split_once('/')?;
    let valid = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    if !valid(owner) || !valid(name) {
        return None;
    }
    let authors: Vec<String> = authors
        .split(',')
        .map(|author| author.trim().to_ascii_lowercase())
        .collect();
    // GitHub App authors log in as `<slug>[bot]`, e.g. `dependabot[bot]`.
    let valid_author = |author: &str| valid(author.strip_suffix("[bot]").unwrap_or(author));
    if authors.iter().any(|author| !valid_author(author)) {
        return None;
    }
    Some((repo.to_ascii_lowercase(), authors))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_allows_nothing_and_says_so() {
        let policy = AdminMergePolicy::parse("");
        let reason = policy
            .allows(Some("sweetgreen/ai-foundry"), Some("davidvasandani"))
            .unwrap_err();
        assert!(reason.contains("disabled"), "{reason}");
    }

    #[test]
    fn entries_match_repo_and_author_case_insensitively() {
        let policy = AdminMergePolicy::parse(
            " sweetgreen/ai-foundry:davidvasandani,Other-Bot ;\nacme/x:alice\tacme/x:bob",
        );
        assert!(
            policy
                .allows(Some("Sweetgreen/AI-Foundry"), Some("DavidVasandani"))
                .is_ok()
        );
        assert!(
            policy
                .allows(Some("sweetgreen/ai-foundry"), Some("other-bot"))
                .is_ok()
        );
        assert!(policy.allows(Some("acme/x"), Some("alice")).is_ok());
        assert!(policy.allows(Some("acme/x"), Some("bob")).is_ok());

        let wrong_author = policy
            .allows(Some("sweetgreen/ai-foundry"), Some("mallory"))
            .unwrap_err();
        assert!(wrong_author.contains("mallory"), "{wrong_author}");
        let wrong_repo = policy
            .allows(
                Some("sweetgreen/terraform-infrastructure"),
                Some("davidvasandani"),
            )
            .unwrap_err();
        assert!(wrong_repo.contains("not in"), "{wrong_repo}");
        assert!(policy.allows(None, Some("davidvasandani")).is_err());
        assert!(policy.allows(Some("acme/x"), None).is_err());
    }

    #[test]
    fn github_app_authors_are_accepted() {
        let policy = AdminMergePolicy::parse("acme/x:alice,renovate[bot]");
        assert!(policy.allows(Some("acme/x"), Some("renovate[bot]")).is_ok());
        assert!(policy.allows(Some("acme/x"), Some("alice")).is_ok());
        assert!(policy.allows(Some("acme/x"), Some("renovate")).is_err());
        // Brackets anywhere else are still malformed.
        assert_eq!(
            AdminMergePolicy::parse("acme/x:[bot] acme/y:re[bot]novate"),
            AdminMergePolicy::default()
        );
    }

    #[test]
    fn malformed_entries_and_wildcards_are_ignored() {
        let policy = AdminMergePolicy::parse(
            "sweetgreen/*:david sweetgreen:david */*:david acme/x: acme/y:* acme/z",
        );
        assert_eq!(policy, AdminMergePolicy::default());
    }
}
