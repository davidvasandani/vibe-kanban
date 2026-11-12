# Research

- **Hashmal access**: `/api/filters/chng` needs a browser session (Django
  `django_auth`), so a server can't call it. `/mcp` takes bearer tokens and is
  stateless (a bare `tools/call` works without `initialize`). Decision: add a
  read-only MCP tool. Alternative (read Hashmal's SQLite directly like
  `daily-report`) rejected: Hashmal runs on another host.
- **Continuation**: VK cannot resume a Claude transcript that lives on
  macmini-m2 (XLVI forbids fabricating vendor history). Decision: seed the new
  session's first prompt with the transcript tail.
- **Settings storage**: `Config` is returned whole by `/api/info`, so secrets
  can't live there. Decision: singleton table + sealed secrets, mirroring
  `github_owner_tokens`. Alternative (reuse a project's remote Jira sync
  credential) rejected: that credential lives on the remote server, is
  project-scoped and org-admin owned.
- **Jira API**: Cloud `/rest/api/3/search/jql` with `nextPageToken`
  (legacy search removed). v3 returns `summary`/`status` as plain fields;
  descriptions are not requested, so ADF is irrelevant.
- **No new dependencies.**
