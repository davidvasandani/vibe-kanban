# Contract: Bundled Slack Catalog Migration

Given a native MCP entry named `slack`:

- If its normalized form exactly equals the historical mutable upstream stdio
  template or the historical pinned-fork `v1.3.0-vk.2` stdio template, return
  the current preconfigured HTTP definition.
- Remove the old `SLACK_MCP_XOXP_TOKEN` from the replacement; do not materialize
  it as a header or environment value.
- Apply normal executor-native HTTP adaptation to the replacement.
- If any command, arg order/value, environment key set, transport field, or extra
  field differs, do not migrate it.
- Reconciliation is idempotent: reading already migrated native definitions
  returns one equivalent logical server with no conflict.

