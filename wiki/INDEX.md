# Project Knowledge Base

Distilled, reusable knowledge about this codebase, written by and for task
agents (and humans). Each page covers one topic and lists the task ids that
contributed to it.

## Conventions

- One topic per page, kebab-case filename, `# Title` heading.
- Every page ends with a `## Contributed by` section listing task ids
  (e.g. `vk/de6e-improve-column-s`) so knowledge can be traced to its task.
- Update an existing page rather than creating a near-duplicate; add your
  task id when you do.
- Keep this index in sync: one line per page.
- Record only knowledge that is reusable beyond the task that produced it —
  architecture decisions, non-obvious gotchas, rejected alternatives and why.
  Plain code structure belongs in `CLAUDE.md`/`AGENTS.md`, not here.

## Pages

- [electric-sync-fallback.md](electric-sync-fallback.md) — Client Electric
  hybrid sync + REST fallback: Electric-first-then-fallback architecture, the
  error/recovery callback chain to the navbar banner, why the readiness timeout
  must fall back silently ("falling back is recovery, not an error"), and the
  stable-callback / caching / testing gotchas.
- [mobile-kanban-scrolling.md](mobile-kanban-scrolling.md) — Mobile kanban
  board scroll/snap architecture: nested single-axis scrollers, the
  `overflow-y` → `overflow-x` promotion gotcha, gesture routing, history of
  fixes.
- [self-hosted-deployment.md](self-hosted-deployment.md) — Versioned-release
  deploy contract (`VK_RELEASES_DIR`), why services must not run from the
  source checkout, deploy-loop invariants (reconciler over edge triggers,
  health-gated rollback, paging), health endpoints, rejected alternatives.
