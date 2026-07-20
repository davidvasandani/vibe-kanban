# Workspace environment inheritance

Tags: `6d24-org-env-vars-are`

## One workspace has multiple process boundaries

“Available in the workspace” is broader than the coding-agent execution path.
Vibe Kanban starts setup scripts, agents, and development servers through
`ContainerService`, but interactive terminals are created independently through
`PtyService`. Configuration inheritance must be audited at every child-process
boundary; fixing only the managed executor path can leave the terminal with a
different environment.

## Resolve once, inject explicitly

Keep tenant lookup and filtering behind one workspace-scoped resolver. The
resolver maps the local workspace through its task/project to the remote project,
performs authenticated organization access, applies a short timeout, filters
reserved names, and degrades to an empty map. Consumers receive the resolved map
explicitly and pass it only to the child process.

Do not write secret `.env` files, mutate the long-lived server environment, or
duplicate remote lookup logic in terminal routes. Those approaches respectively
persist secrets, leak scope across concurrent workspaces, or allow security and
failure behavior to drift.

## Precedence belongs to the execution boundary

Apply inherited organization values before process-owned values. Reserved
application keys (`VK_*`, `PATH`, `HOME`, loader variables, executor auth wiring)
are filtered by the resolver. The PTY then owns terminal semantics such as
`TERM`, `COLORTERM`, prompt configuration, and `VIBE_KANBAN_TERMINAL`, so it
applies those last.

Non-workspace PTYs need an explicit choice too. Managed CLI login sessions pass
an empty workspace map and retain their minimal allowlisted host environment.

## Validation pattern

- Spawn a harmless direct PTY command with a synthetic inherited value and
  assert the child can read it.
- Supply a conflicting terminal-owned key and assert the PTY value wins.
- Unit-test the reserved-name boundary with both rejected runtime keys and
  accepted credential-style keys.
- Never use real credentials or log/debug-format the resolved map.
