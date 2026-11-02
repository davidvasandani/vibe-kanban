# Prior knowledge

Task: f960-update-readme. Read-only recall searched `wiki/INDEX.md` and relevant topic pages before planning. The knowledge base is populated.

- `wiki/self-hosted-deployment.md`: build immutable releases outside the source checkout; homelab owns health-gated rollout and rollback. Current live UI is local-web; remote-web is a separate build target. Check source before repeating historical host details.
- `wiki/external-connector-sync.md`: collaboration issues belong to the remote Postgres stack, not local SQLite tasks. Jira is a shipped bidirectional connector; configuration and credentials are explicit.
- `wiki/task-pipeline-block.md` and `wiki/new-issue-pipeline-defaults.md`: pipelines are TOML-defined prompt stages composed into issue descriptions, with visible stage markers and project/org/machine defaults. They are not a separate guaranteed CI execution engine.
- `wiki/tracked-background-subtasks.md`: independent child workspaces persist results and can resume an exact parent session; commits require explicit integration and a completed turn is not proof of task success.
- `wiki/durable-session-wakeups.md`: persisted timers and poller conditions dispatch follow-up turns at eligible boundaries; automatic archive holds preserve completion intent.
- The index points to browser control arbitration, managed CLIs, MCP OAuth, global and session history search, credential routing, worker recovery and responsive layout. Verify these in their implementation and user docs during the feature audit.

Implications: organize README by user workflows rather than origin; explain local, remote and cluster roles separately; label optional integrations; link focused guides instead of copying operational internals. Avoid unstable model/version inventories and deployment-specific credentials.

The implementation audit also found `docs/knowledge-base/INDEX.md`, a second maintained knowledge index. Its clustered-workspace, prompt-driven pipeline and worktree-formatting pages reinforce the same execution/deployment and verification boundaries. Both indexes are linked from the README.
