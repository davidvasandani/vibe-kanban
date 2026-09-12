# Technical plan: workspace creation reliability

Spec: ./spec.md

## Technical context and approach
Rust/Tokio coordinator, SQLite lifecycle state, shared Git stores. Production failures collected read-only from think2 vibe-kanban-dev journal on 2026-09-12.

1. Reproduce repository lock behavior in crates/worktree-manager/src/worktree_manager.rs: compare process-local path key with SQLite repo-id identity and inspect dropped/acquired lease behavior. Fix demonstrated contention at the ownership boundary, not by retrying workspace startup.
2. Inspect placement transitions in crates/db/src/models/workspace.rs and create_cluster_workspace in crates/local-deployment/src/container.rs. Correlate stored states with failed transition logs; correct demonstrated transition/result handling without weakening compare-and-set.
3. Preserve safe user failure text in crates/server/src/routes/workspaces/create.rs; if needed identify bounded failure categories with workspace identity rather than copying arbitrary errors.
4. Add regression tests alongside affected Rust code. No dependencies or wire schema changes anticipated.
5. Run frozen dependency installation, focused tests, relevant checks, required formatting, independent Codex review, knowledge update, and PR merge.

## Constitution check
II, III, VI: evidence-led minimal corrections with actual regression tests. XV/XVIII/XX: no destructive recovery, no unfenced concurrency, no affinity reassignment or broad Git prune. XXI: diagnostics remain safe and identifying. XXVIII: preserve durable lifecycle identity and no startup replay. No deviations.

## Risks
Request cancellation may abandon a guard while blocking Git continues: merely releasing in Drop is unsafe. Lock lease expiry is not evidence that Git stopped. A failed transition may already have committed: verify actual persisted evidence rather than blindly repeating filesystem work. Scope will be narrowed/refined from tests before edits.
