# Post-Implementation Analysis: CLI Tools in Workspace Sessions

**Task**: `vk/b2a2-add-vk-cli-tools`
**Result**: Independent review passed

## Coverage Cross-Check

| Requirement | Implementation evidence | Validation |
| --- | --- | --- |
| FR-1, FR-6 | Local managed execution uses the shared helper in `crates/local-deployment/src/container.rs`; local workspace terminals add PATH in `crates/server/src/routes/terminal.rs`; worker execution and terminals add it in `crates/worker/src/{execution,terminal}.rs` | Focused crate tests and server check pass |
| FR-2, FR-3, FR-4 | `append_cli_tools_to_path` delegates to ordered, de-duplicating `merge_paths`, with inherited PATH first | Three `utils::shell` tests pass |
| FR-5 | The helper returns `None` when the managed bin directory is absent; callers leave environment unchanged | Missing-directory helper test passes |
| FR-7 | Worker execution and terminal spawners derive their path locally; coordinator dispatch payloads are unchanged | Worker tests and unchanged `cluster-protocol` diff |
| FR-8 | The helper adds only `cli_tools_dir()/bin` | Direct code inspection and assets contract comment |
| FR-9 | Organization environment resolution and reserved-name filtering are unchanged; augmentation happens after scoped values | Existing local PTY precedence tests plus code inspection |
| FR-10 | Only Vibe Kanban source/spec/knowledge artifacts changed; no other service or homelab module changed | `git diff --name-only` |

## Constitution Cross-Check

- II (test the contract): ordering, preservation, deduplication, absence, local
  PTY precedence, worker raw execution, and terminal bounds have automated
  evidence.
- III/VI/XXI (small change, reuse, one convention): one helper reuses the
  canonical asset directory and merge function.
- VIII (managed tools): host paths precede the stable managed bin directory;
  no credentials or staging paths are exposed.
- XVIII/XX (distributed paths): worker-local derivation avoids sending a
  coordinator path across nodes.
- XIV (worktree-safe verification): locked dependencies were installed before
  repository formatting; no verification step silently skipped a crate.

No constitution violation or unresolved gap remains.

## Verification Evidence

- `pnpm install --frozen-lockfile` — passed.
- `cargo test -p utils shell::tests` — 3 passed.
- `cargo test -p local-deployment pty::tests` — 2 passed.
- `cargo test -p worker execution::tests` — 5 passed.
- `cargo test -p worker terminal::tests` — 1 passed.
- `cargo check -p server` — passed.
- `pnpm run format` — passed; all frontend files unchanged.
- `git diff --check` — passed.

No schema, protocol, generated type, dependency, frontend, or deployment change
was introduced.

## Independent Review

`codex review --commit 8a04ea8f` reported no significant findings. It confirmed
that the changes derive the managed directory on the execution host, preserve
PATH ordering and deduplication, no-op safely when absent, and cover the local
and clustered spawn boundaries. The reviewer also recompiled the affected
crates.

## Knowledge Base

Updated `wiki/managed-cli-tool-catalog.md` and `wiki/INDEX.md` with the reusable
workspace PATH and clustered execution-host contract, tagged
`vk/b2a2-add-vk-cli-tools`. The knowledge-base update was committed separately
as `a3becd10`.
