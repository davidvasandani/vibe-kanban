# Tasks — Auto-archive workspaces on terminal status (Done or Cancelled)

**Task**: `vk/2f63-auto-archive-wor` · **Plan**: [`plan.md`](plan.md)

All work is in one file (`crates/remote/src/routes/issues.rs`), so the code
tasks are **not** parallel-safe with each other (they edit the same file); the
final checks touch different tooling and are marked `[P]`.

| ID | Task | File(s) | Depends on |
| --- | --- | --- | --- |
| T001 | Add pure helper `terminal_status_name(&str) -> Option<&'static str>` ("Done"; "Cancelled"/"Canceled"; case-insensitive; else None) | `crates/remote/src/routes/issues.rs` | — |
| T002 | Rename `archive_workspaces_for_done_issue` → `archive_workspaces_for_terminal_issue`; swap the `name == "Done"` check for `terminal_status_name`; bind `terminal` label; gate the unmerged-PR `warn!` behind `terminal == "Done"`; add info log with status + workspace count; update doc comment | `crates/remote/src/routes/issues.rs` | T001 |
| T003 | Update the two call sites (`update_issue`, `bulk_update_issues`) to the renamed function | `crates/remote/src/routes/issues.rs` | T002 |
| T004 | Add `#[cfg(test)] mod tests` covering `terminal_status_name` (Done/done/DONE; Cancelled/cancelled/Canceled; negatives: "In progress","Backlog","To do","") | `crates/remote/src/routes/issues.rs` | T001 |
| T005 [P] | `cargo test -p remote` (new unit test green) | — | T003, T004 |
| T006 [P] | `cargo clippy -p remote` / `pnpm run backend:check` clean | — | T003 |
| T007 [P] | `pnpm run format` | — | T003, T004 |

## Definition of done
All acceptance criteria in [`spec.md`](spec.md) hold; T005–T007 pass; Done
behaviour is byte-for-byte preserved; Cancelled newly archives active workspaces
inside the update transaction.
