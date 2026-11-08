# /speckit.tasks

- [x] T001 Trace admission and write spec, prior knowledge and plan in SPEC.md, IMPLEMENTATION_PLAN.md and specs/vk/0890-investigate-a-fo/.
- [x] T002 Establish server launch exclusion and regression tests in crates/services/src/services/container.rs and crates/db/src/models/execution_process.rs.
- [x] T003 Integrate busy follow-up queueing and dispatcher admission in crates/server/src/routes/sessions/mod.rs and crates/server/src/issue_comment_delivery.rs; audit queue handoffs.
- [x] T004 Add independent worker session guard and tests in crates/worker/src/execution.rs.
- [x] T005 Disable native peer discovery/delivery and add identity guidance/tests in crates/executors/src/executors/claude.rs and claude/client.rs.
- [x] T006 Run focused tests, format and checks; obtain independent Codex review and resolve findings.
- [x] T007 Document reusable behavior and stable identity in wiki/agent-process-lifecycle.md and wiki/INDEX.md; commit wiki.
- [ ] T008 Open PR, verify required checks, and merge.

T002 precedes T003. T004/T005 can be developed independently but are intentionally serial
in this workspace. T006 depends on T002–T005; T007 and T008 follow validation/review.
