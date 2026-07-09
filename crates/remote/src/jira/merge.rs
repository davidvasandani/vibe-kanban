//! Per-field 3-way merge between Jira and VK against the link's last-synced
//! snapshot.
//!
//! Comparing each side against the snapshot (instead of comparing clocks on
//! every pass) means: no cross-system clock reads in the common case, and the
//! reconciler's own writes — which update the snapshot in the same
//! transaction — can never be re-detected as user edits (FR-18). Timestamps
//! are consulted only when *both* sides changed the same field, and ties go
//! to Jira, the team-visible source of truth (FR-15).
//!
//! Known v1 limitation (recorded in the spec analysis): the VK timestamp is
//! issue-level (`issues.updated_at`), so an unrelated VK edit (sort order,
//! priority) can win a conflict for a field it didn't touch. Conflicts
//! require both sides to have changed the same field since the last sync, so
//! the blast radius is small.

use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldAction {
    /// Both sides match the snapshot (or each other): nothing to do.
    NoOp,
    /// Only Jira moved: write the Jira value to VK.
    WriteVk,
    /// Only VK moved: write the VK value to Jira.
    WriteJira,
}

/// Decide what to do about one synced field.
///
/// `jira_matches_snapshot` / `vk_matches_snapshot` compare the side's current
/// value to the link snapshot. `jira_updated` is the issue's `updated` field
/// from Jira; `vk_updated` is `issues.updated_at`.
pub fn decide_field(
    jira_matches_snapshot: bool,
    vk_matches_snapshot: bool,
    jira_updated: Option<DateTime<Utc>>,
    vk_updated: DateTime<Utc>,
) -> FieldAction {
    match (jira_matches_snapshot, vk_matches_snapshot) {
        (true, true) => FieldAction::NoOp,
        (false, true) => FieldAction::WriteVk,
        (true, false) => FieldAction::WriteJira,
        // Conflict: last write wins; unorderable or tied -> Jira wins.
        (false, false) => match jira_updated {
            Some(jira_updated) if vk_updated > jira_updated => FieldAction::WriteJira,
            _ => FieldAction::WriteVk,
        },
    }
}

/// A field where both sides already hold the same value is converged no
/// matter what the (possibly stale) snapshot says; callers should test this
/// first and treat it as a snapshot refresh rather than a write.
pub fn values_equal<T: PartialEq>(jira: &T, vk: &T) -> bool {
    jira == vk
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn t(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(secs, 0).unwrap()
    }

    #[test]
    fn full_decision_table() {
        // (jira_matches, vk_matches) -> action
        assert_eq!(decide_field(true, true, Some(t(10)), t(20)), FieldAction::NoOp);
        assert_eq!(
            decide_field(false, true, Some(t(10)), t(20)),
            FieldAction::WriteVk
        );
        assert_eq!(
            decide_field(true, false, Some(t(30)), t(20)),
            FieldAction::WriteJira
        );
    }

    #[test]
    fn conflict_last_write_wins() {
        // VK edit is newer -> VK wins.
        assert_eq!(
            decide_field(false, false, Some(t(10)), t(20)),
            FieldAction::WriteJira
        );
        // Jira edit is newer -> Jira wins.
        assert_eq!(
            decide_field(false, false, Some(t(30)), t(20)),
            FieldAction::WriteVk
        );
    }

    #[test]
    fn conflict_ties_and_unknown_jira_time_go_to_jira() {
        assert_eq!(
            decide_field(false, false, Some(t(20)), t(20)),
            FieldAction::WriteVk
        );
        assert_eq!(
            decide_field(false, false, None, t(20)),
            FieldAction::WriteVk
        );
    }
}
