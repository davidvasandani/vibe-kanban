# Implementation Plan: Reliable MCP Reload

1. Reproduce the reported behavior against the current session/execution
   lifecycle, including reloads made while idle, while a Codex execution is
   starting, and while a control from the previous execution is being cleaned
   up.
2. Add a focused regression test at the narrowest failing boundary. Model the
   real order of refresh request, execution start, Codex control publication,
   inventory enumeration, and execution cleanup.
3. Correct refresh coordination so a session-scoped pending generation is
   consumed only by the first execution that can safely adopt it. Preserve the
   existing next-boundary confirmation rule for requests that race a turn whose
   configuration is already resolved.
4. Ensure every relevant Codex startup path either confirms the generation or
   transitions it to a safe terminal failure. Prevent an old execution cleanup
   from deleting a newer usable control.
5. If investigation shows the browser is losing or suppressing the request,
   repair the toolbar action and polling lifecycle so it is session-isolated,
   retryable after terminal outcomes, and explicit about queued versus applied
   state.
6. Regenerate shared types only if the backend contract changes; do not edit
   generated files directly.
7. Run the new regression test plus targeted executor, service,
   local-deployment, server, and web checks. Install locked frontend
   dependencies first if they are absent, then run repository formatting and
   inspect the resulting diff for unrelated changes.
8. Run the SpecKit analysis and implementation task checklist to closure, then
   run an independent Codex diff review. Address confirmed significant findings
   and repeat targeted verification and review until clear.
9. Update `docs/knowledge-base/active-mcp-refresh.md` with the reusable lifecycle
   lesson, tag it with task `9151-reloading-mcp-no`, refresh the knowledge index,
   and commit the knowledge-base update before handoff.

## Expected Change Surface

- Primary: `crates/local-deployment/src/container.rs` and its tests.
- Possible protocol boundary: `crates/executors/src/executors/codex.rs` or
  `crates/executors/src/executors/codex/client.rs`.
- Possible UI boundary:
  `packages/web-core/src/features/workspace-chat/ui/SessionChatBoxContainer.tsx`.
- Documentation/spec artifacts under `specs/vk/9151-reloading-mcp-no/` and
  `docs/knowledge-base/`.
- Deployment module only if the defect is proven to originate in how this Vibe
  Kanban build is packaged or configured.
