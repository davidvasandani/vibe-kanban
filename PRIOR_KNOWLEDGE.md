# Prior knowledge — vk/0773-debug-token-erro

Searched `wiki/INDEX.md` and project knowledge pages for Codex
authentication, refresh tokens, stderr normalization, setup-required errors,
and worker stream boundaries.

## Relevant pages

### `wiki/codex-credential-refresh.md`

- ChatGPT OAuth refresh tokens rotate and are single-use. Concurrent Codex
  processes historically raced when sharing `auth.json`; Vibe Kanban now does
  a serialized, up-front refresh to reduce those races.
- Cluster workers intentionally receive a blank refresh token and use
  `CODEX_REFRESH_TOKEN_URL_OVERRIDE` to fetch the canonical node's current
  access token. A worker should not attempt OpenAI refresh directly.
- Structured v2 and legacy Codex error notifications already route messages
  containing `Failed to refresh token` or `access token could not be refreshed`
  through `refresh_failure_setup_message` and classify them as
  `SetupRequired`.
- Broker endpoints deliberately avoid 401 and known Codex refresh error codes
  so their actionable message survives Codex's own canned error mapping.

### `wiki/worker-journal-agent-stream-boundary.md`

- Agent stderr remains agent stderr across worker journaling; only agent stdout
  enters vendor protocol parsers. This symptom belongs in the Codex stderr
  normalizer, not the worker journal classifier.
- Unknown structured worker metadata is converted to a stderr diagnostic and
  must not be mistaken for vendor output.

### `wiki/unattended-reauth.md`

- Authentication recovery must be based on affirmative auth-error evidence,
  not merely a nonzero exit status.
- Error reporting must avoid leaking credential material and should preserve an
  actionable, coarse cause.

## Implications for this task

- Reuse the existing refresh-failure classifier and guidance so structured and
  stderr failures have one recovery contract.
- Normalize only positively identified Codex refresh failures; preserve all
  unrelated stderr.
- Apply suppression/deduplication at the stderr record boundary, where the
  repeated logger output is produced, without weakening the worker stream
  boundary.
- Keep both recovery modes in the guidance: repair the configured cluster
  refresh endpoint/credential owner or run `codex login` for a direct login.

The knowledge base is populated. This stage made no changes to it. The initial
`SPEC.md` predates this recall as required by the pipeline stage order; these
constraints refine the implementation plan.
