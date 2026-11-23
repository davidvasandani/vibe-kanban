# SpecKit upstream baseline

Vibe Kanban's embedded SpecKit scaffold is refreshed from:

- release: `v1.4.3`
- commit: `980b3d027f4bd7958c35f97f618d854b41320e3f`
- source: <https://github.com/github/spec-kit/releases/tag/v1.4.3>

## Supported upstream surface

VK adapts the core `spec`, `plan`, and `tasks` templates and the semantic flow
of the seven stages in `assets/pipelines/speckit.toml`. The templates retain
upstream's content structure while using VK's `<FEATURE_DIR>` placeholder and
literal `/speckit.*` command names.

VK intentionally does not vendor SpecKit's branch-management scripts, optional
extensions, integrations, `checklist`, or `converge` commands. VK persists the
authoritative feature key and spec-host repository itself; upstream scripts
that create or derive git branches would conflict with that state model.

`memory/constitution.md` is a populated VK-safe project default, not upstream's
empty constitution placeholder. Operator-edited constitutions remain owned by
the repository.

## Refresh procedure

1. Check out the immutable release tag and record its resolved commit above.
2. Diff `templates/` against the previously recorded release.
3. Refresh the three supported templates, adapting only VK path/command
   placeholders.
4. Preserve each replaced VK template byte-for-byte under `historical/` and
   register it in the service's exact-predecessor migration table.
5. Keep unknown or customized workspace files unchanged; only exact historical
   defaults migrate.
6. Run the focused SpecKit service tests and independent diff review.
