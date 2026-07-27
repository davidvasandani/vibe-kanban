# Implementation Plan: Verified Slack MCP installation

**Spec**: `./spec.md`
**Status**: Ready

## Technical Context

- Rust catalogue contract:
  `crates/executors/default_mcp.json` and
  `crates/executors/src/mcp_config.rs`
- GitHub Actions audit:
  `.github/workflows/pinned-artifacts.yml`
- User documentation:
  `docs/integrations/mcp-server-configuration.mdx`
- Maintainer knowledge:
  `docs/knowledge-base/forked-mcp-server-packaging.md`
- Dependency automation: `renovate.json`
- Baseline dependency: commit `2e4b77aa` from task
  `36d7-use-the-maintain`

The chosen posture does not add a runtime dependency or change the connector
command. It documents the temporary exception, reduces the audit interval from
weekly to daily, and makes audit failure create or update a durable GitHub
issue.

## Architecture & Approach

1. Incorporate predecessor commit `2e4b77aa`, preserving this task's pipeline
   documents and the current branch's work-preservation constitution principle.
2. Keep the exact fork release URL in `default_mcp.json`, because no authorised
   preventative package source exists yet.
3. Keep `SLACK_MCP_LAUNCHER_SHA256` and
   `slack_pinned_launcher_matches_recorded_digest`: they remain the audit's
   source of truth.
4. Change the scheduled workflow cron from weekly to daily.
5. Give the audit job only `contents: read` and `issues: write`.
6. After the digest-test step, run a failure-only notification step that uses
   the GitHub API and `github-script` to:
   - find an open issue with a stable label/title;
   - add a comment containing the run URL if one exists; or
   - create a new issue otherwise.
   The notification step itself must run even when the digest check fails.
7. Update integration documentation to say the outer launcher is audited rather
   than verified before execution, and link the accepted-risk trigger.
8. Extend the fork-packaging knowledge page with a dated decision record,
   rationale, residual attack, notification flow, and reopening condition.
9. Keep Renovate on `github-releases`; refine reviewer notes if needed so no
   text implies the outer digest is enforced during install.
10. Verify the existing catalogue/adaptation tests, ignored network digest
    test, workflow syntax, Renovate config, formatting, and a clean-cache MCP
    handshake.

## Data Model

See `./data-model.md`. No application persistence changes are introduced. The
only durable state added at runtime is a GitHub issue representing an active
audit failure.

## Contracts

See `./contracts/audit-notification.md`.

## Research Notes

See `./research.md`.

## Constitution Check

- I / Clarity: documentation calls the control detection, not prevention.
- II / Test contract: focused catalogue tests, the real network digest audit,
  workflow validation, and clean-cache handshake are explicit.
- III / Small reversible steps: no new installation architecture is introduced
  while the one-line preventative route is externally blocked.
- VI / Don't rebuild: the predecessor audit and existing workflow are extended.
- VIII / Managed tools: no incomplete managed-tool entry is added.
- XIV / Worktree-safe verification: locked dependency setup precedes formatting.
- XVI / Bundled delivery: this uses the principle's explicit exception path,
  with threat, control, notification, and reopening trigger committed.

No constitution violation remains open. The feature deliberately does not claim
prevention; it satisfies the constitution's documented-exception branch.

## Risks & Dependencies

- The GitHub repository must permit Actions to create issues with the scoped
  token. A repository policy disabling issue writes will cause the notification
  step to fail visibly in the same workflow.
- Daily schedules are best-effort GitHub Actions schedules and can be delayed.
  Documentation therefore states the target interval rather than promising an
  exact 24-hour maximum.
- The genuine attachment retrieval check depends on a valid Slack token and a
  real workspace attachment. These are not committed or fabricated.
- Preventative closure still depends on maintainers obtaining an npm package
  and configuring trusted publication.

