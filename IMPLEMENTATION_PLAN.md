# Implementation plan: Codex Slack MCP and Azure/Entra capabilities

**Task:** `vk/84ef-restore-slack-mc`

1. Establish the current capability boundaries.
   - Trace shared MCP persistence, Codex native materialization, fresh-session
     launch, clustered dispatch, and the existing MCP reload/restart action.
   - Trace app-managed CLI installation and PATH construction for local agents,
     remote workers, setup/dev processes, and workspace PTYs.
   - Inspect the Vibe Kanban Nix module for Slack URL convergence and any
     existing Azure package/auth state.

2. Define truthful capability diagnostics.
   - Add a backend read model that compares configured Slack assignment,
     executor-native persistence, active-session MCP status/tool inventory, `az`
     executable discovery, and Azure account probe state without returning
     credentials or raw subprocess output.
   - Represent unavailable, stale/restart-required, unauthenticated, connected,
     and failed states distinctly with allowlisted remediation.
   - Surface the result beside the existing MCP refresh/restart workflow.

3. Restore Slack for new and active Codex sessions.
   - Ensure settings-owned Slack HTTP MCP definitions reach the exact Codex home
     used by fresh local and remote executions.
   - Reuse the supported Codex reload/next-turn confirmation where available and
     the existing safe agent-restart fallback otherwise.
   - Make reconnect completion invalidate/refetch the relevant state so the
     refresh action is discoverable without recreating the task.

4. Restore Azure CLI and read-only Entra authentication.
   - Put the pinned/host-managed Azure CLI on the supervised coordinator and
     worker agent PATH, retaining the app-managed CLI fallback where applicable.
   - Reuse the existing durable Azure CLI authentication model and expose only
     non-secret account status to agents/diagnostics. If deployment provisioning
     is required, load it through systemd credentials or an equivalent protected
     runtime store and grant only read-only Graph device permissions.
   - Ensure the same context is available at each actual workspace execution
     boundary, including execution-scoped homes where vendor state must be
     linked rather than copied.

5. Add regression coverage.
   - Backend tests: configured/connected Slack with absent native/live agent
     state, stale state requiring refresh, successful tool registration, missing
     `az`, unauthenticated Azure, and authenticated secret-free probe output.
   - Executor/cluster tests: fresh Codex config materialization and refresh
     rematerialization on the owning worker.
   - Nix tests: Azure CLI appears in coordinator and worker service PATH and
     runtime auth wiring contains references/paths but no credential contents.
   - Frontend tests: mismatch diagnostic and supported refresh action rendering.

6. Document and verify.
   - Document required Slack app/connection permissions, Codex assignment,
     Azure login/auth ownership and minimum Graph permissions, restart/refresh
     steps, and layered troubleshooting.
   - Run focused Rust/TypeScript/Nix tests, formatting, generated-type checks,
     and broader checks proportionate to touched code.
   - Validate read-only exact-hostname searches for the four supplied candidates
     across Slack and Entra, then correlate with LogMeIn when the live connected
     capabilities are available.

7. Review and ship.
   - Run an independent Codex diff review, fix confirmed findings, rerun relevant
     checks until no significant findings remain, update the project knowledge
     base and index, commit both repositories as needed, then open and merge the
     pull request(s) against their base branches.
