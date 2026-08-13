# Implementation Plan: Low-Disk Warnings and Issue Follow-Through

1. Trace the existing metrics transport, filesystem DTO, right-sidebar
   collapsible lifecycle, remote issue mutation, database schema, and Nix
   service configuration. Record exact extension seams in the SpecKit research
   and plan artifacts.
2. Define validated low-disk threshold configuration with documented defaults:
   warning `<10%` or `<5 GiB`; critical `<2%` or `<1 GiB`. Expose effective
   values with the coordinator metrics contract and wire deployment overrides
   only through `homelab/modules/vibe-kanban-rebuild.nix` if required.
3. Add a pure shared classification model that safely derives filesystem and
   node severity, honors the conservative OR rule, rejects absent/invalid
   samples, applies critical precedence, and returns the worst affected
   filesystem plus rollup counts. Cover exact boundaries and mixed nodes.
4. Extend the Server Metrics UI with accessible row warnings containing icon,
   severity text, filesystem, available capacity, use percentage, and
   mountpoint. Add keyboard-safe activation, pending/error states, and
   light/dark theme-compatible styling.
5. Add a header-owned metrics subscriber that reuses the existing cache/query
   identity and renders the worst severity plus affected-node count outside the
   collapsible body. Preserve the existing rule that the live socket/detail
   container is unmounted while collapsed.
6. Add a coordinator resolve-or-create-low-disk-issue API. Validate that the
   node/sample is current and authorized, generate canonical permanent-fix
   Markdown, atomically reuse an existing open node incident, or create a new
   issue in the selected/linked project. Return the issue ID and whether it was
   created.
7. Persist machine-readable incident identity and enforce one open low-disk
   issue per node under concurrent requests. Permit a new issue after the old
   one reaches a terminal/closed status. Add migrations and database/API tests.
8. Connect warning activation to the API and navigate to the returned issue.
   Add component tests for create/reuse, double activation, collapsed rollup,
   facts, accessibility, and failure recovery.
9. Regenerate derived types where source types changed, format, and run focused
   frontend/backend/configuration tests followed by the repository's standard
   checks in proportion to available disk and time.
10. Run the mandated independent Codex review, fix confirmed findings, repeat
    until no significant findings remain, then document reusable architecture
    knowledge and update its index.
11. Commit the implementation and knowledge-base updates intentionally, push
    the task branch, open a pull request against the detected base branch,
    monitor required checks, fix failures, and merge only when green.

