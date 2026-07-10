//! Status mapping between Jira workflow statuses and VK board columns.
//!
//! Jira -> VK: explicit per-status-name override, else a default keyed on
//! Jira's status *category* (the only workflow-independent signal Jira
//! gives us). VK -> Jira: explicit table only — pushing a transition to the
//! wrong workflow status is worse than not pushing, so there is no guessy
//! fallback in that direction; missing entries are seeded from observed
//! statuses and otherwise reported as "not propagated".

use super::types::JiraStatusMapping;

/// Jira status-category keys (stable across all Jira workflows).
pub const CATEGORY_NEW: &str = "new";
pub const CATEGORY_INDETERMINATE: &str = "indeterminate";
pub const CATEGORY_DONE: &str = "done";

/// Default VK board columns targeted per Jira status category. These are the
/// names seeded for every project (`DEFAULT_STATUSES` in
/// `db/project_statuses.rs`); projects with renamed columns use overrides.
fn category_default(category: &str) -> Option<&'static str> {
    match category {
        CATEGORY_NEW => Some("To do"),
        CATEGORY_INDETERMINATE => Some("In progress"),
        CATEGORY_DONE => Some("Done"),
        _ => None,
    }
}

/// Resolve the VK status name a Jira status should land on.
pub fn resolve_jira_to_vk(
    mapping: &JiraStatusMapping,
    jira_status: &str,
    jira_category: Option<&str>,
) -> Option<String> {
    if let Some(vk) = mapping
        .jira_to_vk
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(jira_status))
        .map(|(_, vk)| vk.clone())
    {
        return Some(vk);
    }
    jira_category.and_then(category_default).map(str::to_string)
}

/// Resolve the Jira status name a VK status change should transition to.
pub fn resolve_vk_to_jira(mapping: &JiraStatusMapping, vk_status: &str) -> Option<String> {
    mapping
        .vk_to_jira
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(vk_status))
        .map(|(_, jira)| jira.clone())
}

/// Seed missing `vk_to_jira` entries from the Jira statuses observed in the
/// query results: each observed status is mapped forward to its VK column,
/// and that column gains a reverse entry if it has none yet. First
/// observation wins; existing (user-edited) entries are never overwritten.
/// Returns `true` when anything changed.
pub fn seed_vk_to_jira(
    mapping: &mut JiraStatusMapping,
    observed: &[(String, Option<String>)],
) -> bool {
    let mut changed = false;
    for (jira_status, category) in observed {
        let Some(vk_status) = resolve_jira_to_vk(mapping, jira_status, category.as_deref()) else {
            continue;
        };
        let exists = mapping
            .vk_to_jira
            .keys()
            .any(|name| name.eq_ignore_ascii_case(&vk_status));
        if !exists {
            mapping.vk_to_jira.insert(vk_status, jira_status.clone());
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping(jira_to_vk: &[(&str, &str)], vk_to_jira: &[(&str, &str)]) -> JiraStatusMapping {
        JiraStatusMapping {
            jira_to_vk: jira_to_vk
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect(),
            vk_to_jira: vk_to_jira
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect(),
        }
    }

    #[test]
    fn override_beats_category_default() {
        let m = mapping(&[("In Review", "In review")], &[]);
        assert_eq!(
            resolve_jira_to_vk(&m, "In Review", Some(CATEGORY_INDETERMINATE)),
            Some("In review".to_string())
        );
    }

    #[test]
    fn override_lookup_is_case_insensitive() {
        let m = mapping(&[("In Review", "In review")], &[]);
        assert_eq!(
            resolve_jira_to_vk(&m, "in review", None),
            Some("In review".to_string())
        );
    }

    #[test]
    fn falls_back_to_category_default() {
        let m = JiraStatusMapping::default();
        assert_eq!(
            resolve_jira_to_vk(&m, "Weird Custom Status", Some(CATEGORY_NEW)),
            Some("To do".to_string())
        );
        assert_eq!(
            resolve_jira_to_vk(&m, "Doing Things", Some(CATEGORY_INDETERMINATE)),
            Some("In progress".to_string())
        );
        assert_eq!(
            resolve_jira_to_vk(&m, "Shipped", Some(CATEGORY_DONE)),
            Some("Done".to_string())
        );
    }

    #[test]
    fn unknown_category_and_no_override_is_none() {
        let m = JiraStatusMapping::default();
        assert_eq!(resolve_jira_to_vk(&m, "Mystery", None), None);
        assert_eq!(resolve_jira_to_vk(&m, "Mystery", Some("undefined")), None);
    }

    #[test]
    fn vk_to_jira_is_explicit_only() {
        let m = mapping(&[], &[("Done", "Closed")]);
        assert_eq!(resolve_vk_to_jira(&m, "Done"), Some("Closed".to_string()));
        assert_eq!(resolve_vk_to_jira(&m, "done"), Some("Closed".to_string()));
        assert_eq!(resolve_vk_to_jira(&m, "In progress"), None);
    }

    #[test]
    fn seeding_fills_missing_reverse_entries_without_overwriting() {
        let mut m = mapping(&[], &[("Done", "Resolved")]);
        let observed = vec![
            ("To Do".to_string(), Some(CATEGORY_NEW.to_string())),
            (
                "In Progress".to_string(),
                Some(CATEGORY_INDETERMINATE.to_string()),
            ),
            ("Closed".to_string(), Some(CATEGORY_DONE.to_string())),
        ];
        let changed = seed_vk_to_jira(&mut m, &observed);
        assert!(changed);
        // Existing user entry survives.
        assert_eq!(m.vk_to_jira.get("Done"), Some(&"Resolved".to_string()));
        assert_eq!(m.vk_to_jira.get("To do"), Some(&"To Do".to_string()));
        assert_eq!(
            m.vk_to_jira.get("In progress"),
            Some(&"In Progress".to_string())
        );
        // Second seeding is a no-op.
        assert!(!seed_vk_to_jira(&mut m, &observed));
    }
}
