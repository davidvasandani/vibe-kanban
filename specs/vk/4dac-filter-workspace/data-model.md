# Data Model: Filter sidebar workspaces by issue status

## Persisted (UI-preferences scratch)

`WorkspaceFilterStateData` (`crates/db/src/models/scratch.rs`)

| Field | Type | Default | Notes |
|-------|------|---------|-------|
| `project_ids` | `Vec<String>` | `[]` | existing |
| `pr_filter` | `WorkspacePrFilterData` | `all` | existing |
| `hidden_issue_status_names` | `Vec<String>` | `[]` | **new**. Normalized (trimmed, lower-cased) status names, plus the optional `__no_issue__` sentinel. `#[serde(default)]`. |

TS store mirror (`useUiPreferencesStore.ts`): `WorkspaceFilterState`
gets `hiddenIssueStatusNames: string[]`.

## Derived at runtime (not persisted)

- `RemoteWorkspaceLink { projectId: string; issueId: string | null }`, keyed by
  local workspace id (from `useUserContext().workspaces` where
  `local_workspace_id` is set).
- `issueStatusNameById: Map<issueId, normalizedName>`. For each issue in the
  linked projects, the name of its status (`issue.status_id` →
  `ProjectStatus.name`). Issues whose status row hasn't loaded are absent.
- `IssueStatusFilterOption { value: normalizedName; label: string }`. One per
  distinct normalized name, ordered by min `sort_order` then label, followed
  by the stale hidden names.

## Relationships

```
SidebarWorkspace.id ──(local_workspace_id)── remote Workspace ──issue_id──> Issue ──status_id──> ProjectStatus.name
```
