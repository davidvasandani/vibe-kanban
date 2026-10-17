# Global Search Contract Extension

`GET /v1/global-search?q={query}` may return an additional result variant:

```json
{
  "kind": "issue",
  "id": "<issue UUID>",
  "title": "Searching by Issue ID should work",
  "context": "VAS-602 · Vibe Kanban / Vibe Kanban",
  "snippet": "",
  "organization_id": "<organization UUID>",
  "project_id": "<project UUID>",
  "workspace_id": null,
  "issue_id": "<issue UUID>",
  "archived": false
}
```

Issue results are membership scoped, limited to 20 returned rows with a 21st
internal sentinel setting the response's existing `truncated` flag, and selected
through `/projects/{project_id}/issues/{issue_id}`.

All existing response fields and result variants remain compatible.
