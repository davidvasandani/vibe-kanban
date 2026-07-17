# Implementation Plan: Organization Environment Variables in Workspace Terminals

1. Add organization environment resolution to the shared `ContainerService`
   interface and delegate to the existing local resolver.
2. Preserve the resolver's remote-project mapping, five-second timeout,
   best-effort fallback, reserved-name filtering, and value-redacted warnings.
3. Extend `PtyService` command creation to accept an explicit environment map.
4. Apply the supplied map before PTY-owned variables so runtime terminal values
   retain precedence.
5. Keep managed CLI login PTYs unchanged by passing an empty explicit map.
6. Resolve the environment for the loaded workspace in the signed terminal
   WebSocket route and carry it into PTY creation.
7. Test child-process visibility, PTY precedence, and reserved-name filtering.
8. Run Rust formatting, focused tests, the server check, and diff whitespace
   validation.
9. Run an independent Codex diff review; address findings and re-verify until
   no significant findings remain.
10. Record the reusable execution-boundary lesson in the project knowledge base
    and refresh its index.
