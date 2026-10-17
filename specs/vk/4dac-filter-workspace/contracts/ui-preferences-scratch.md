# Contract: UI-preferences scratch, `workspace_filters`

```json
{
  "workspace_filters": {
    "project_ids": ["<remote project uuid>"],
    "pr_filter": "all",
    "hidden_issue_status_names": ["in review", "__no_issue__"]
  }
}
```

- `hidden_issue_status_names` is optional on read and defaults to `[]`. A
  payload saved before this feature still loads, with nothing hidden.
- The client always writes it.
- Values are normalized names (`trim().toLowerCase()`) or the `__no_issue__`
  sentinel. The server does not validate the contents.
