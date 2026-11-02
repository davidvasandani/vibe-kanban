# /speckit.plan: README product guide

Spec: ./spec.md. Status: ready.

## Technical context
Markdown documentation for a Rust backend and React/TypeScript frontend monorepo. Local SQLite execution and remote Postgres collaboration are distinct. Build entrypoint is local-build.sh; scripts and packageManager in package.json govern development. Homelab modules/vibe-kanban-rebuild.nix owns the managed service deployment.

## Approach
1. Audit docs/workspaces, docs/settings, docs/integrations, docs/self-hosting, crates/executors/src/executors.rs, crates/server/src/routes, crates/remote/src, crates/mcp/src and packages/web-core. Cross-check README's historical additions against current implementations.
2. Record a concise source map in research.md. Distinguish optional features and historical notes.
3. Replace README.md with a workflow-oriented introduction, comprehensive capability sections, supported-agent list, source quickstart, architecture/component table, self-hosting notes, development commands and documentation pointers. Use one footnote for lineage.
4. Verify relative links, script names, toolchain versions and claims. Run pnpm install --frozen-lockfile and pnpm run format. No application tests needed for prose-only changes; CI remains authoritative for merge.
5. Independent Codex review of the task diff. Address confirmed findings, then validate again.
6. Write a reusable wiki topic on README evidence and distribution/deployment boundaries, refresh wiki/INDEX.md and commit.
7. Open and merge PR against main after inspecting required checks.

## Data model and contracts
Not applicable: no runtime, schema, API or generated type changes.

## Research
See research.md, populated during implementation audit.

## Constitution check
I: plain readable prose. II: source/link/command acceptance checks. III/VI: reuse focused docs and limit edits to documentation. XIV: install locked dependencies before required formatting. No deviations.

## Risks
Stale documentation and inherited examples can misrepresent install, hosting or features. Prefer source for disputed claims. Keep provider model inventories and detailed deployment procedures in their existing guides.
