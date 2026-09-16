# Data Model: Search by Issue ID

No schema change is required. The feature reads existing entities:

| Entity | Fields used | Purpose |
|---|---|---|
| `organizations` | `id`, `name` | Membership scope and display context |
| `organization_member_metadata` | `organization_id`, `user_id` | Authorization boundary |
| `projects` | `id`, `name`, `organization_id` | Issue ownership and route context |
| `issues` | `id`, `project_id`, `simple_id`, `title` | Match, display, and route identity |

## Result projection

An issue search row uses:

- `kind`: `issue`
- `id`: issue UUID
- `title`: issue title
- `context`: `simple_id · organization / project`
- `organization_id`: owning organization UUID
- `project_id`: owning project UUID
- `issue_id`: issue UUID
- `workspace_id`: null
- `snippet`: empty
- `archived`: false (issues have no archive field in this model)
