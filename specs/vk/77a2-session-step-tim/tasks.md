# Tasks

- [x] T001 [P] Add opt-in timestamp enrichment and Rust regressions in `crates/utils/src/msg_store.rs`.
- [x] T002 [P] Remove misleading fallback and add rendered regressions in `packages/web-core/src/features/workspace-chat/ui/ConversationTimestamp.tsx` and `.test.tsx`.
- [x] T003 Wire fresh execution constructors in `crates/local-deployment/src/container.rs` (depends on T001).
- [x] T004 Install locked dependencies; run tests, checks, lint and formatting; document results in `specs/vk/77a2-session-step-tim/validation.md` (depends on T001–T003).
- [x] T005 Independent Codex CLI review; fix confirmed findings and re-verify (depends on T004).
- [x] T006 Update timestamp knowledge topic and index in `docs/knowledge-base/`, commit knowledge (8f518fca) and changes (depends on T005).
- [x] T007 Open PR #372 and hand off to the mandatory current-head CI merge gate (depends on T006).

Pipeline stage 13 merges through Vibe Kanban after all final-head checks pass;
GitHub PR #372 is the authoritative merge completion record.
