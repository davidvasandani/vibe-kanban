# Research: Reliable Parallel Sub-Agent Pipeline

## Failure source

The bundled pipeline is prompt-driven; no Vibe Kanban orchestration engine
interprets provider names. The current fan-out sentence says only to launch
three sub-agents and collect responses. It does not preserve read tools,
identify initial-prompt ordering, distinguish launch attempts from completed
rounds, or forbid synthesized substitute responses. The reported failure is
therefore consistent with an underspecified executable prompt.

Decision: correct and test the bundled contract rather than add a provider
runtime. This follows the original pipeline architecture and constitution
principle IX.

## Read-only child execution

Claude, Codex, and Grok expose different permission and sandbox surfaces, and
those flags can evolve. All three nevertheless need repository-reading tools
for grounded technical analysis.

Decision: specify the semantic policy—retain workspace-reading tools, do not
edit, use a read-only sandbox/permission mode when available, never disable all
tools—without embedding CLI flag spellings.

Rejected alternative: pass an empty tool list. This directly causes the Codex
decline and prevents workspace-grounded analysis.

Rejected alternative: hard-code current CLI commands. That couples a data asset
to volatile provider syntax and duplicates executor integration knowledge.

## Existing-install refresh

The seed manifest records bundled filenames, not content versions. Once
`parallel-subagents.toml` is known, merely changing the embedded asset will not
touch its on-disk copy. Repository history provides the exact single prior
version.

Decision: recognize only those historical bytes and atomically replace them.
Any byte difference is customization and is preserved.

Rejected alternative: overwrite every known bundled file. Pipeline TOMLs are
user-editable and this would destroy custom workflows.

Rejected alternative: version every bundle in the manifest now. It adds schema
and migration machinery beyond the one historical correction needed. Exact
legacy-content migration is smaller and fail-safe.

## Dependencies

No new dependency is needed. Existing standard-library I/O, seed lock, unique
temporary naming, file sync, and cross-platform replace helper cover the
migration.
