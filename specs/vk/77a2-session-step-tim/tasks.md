# Tasks

- [x] T001 [P] Add opt-in timestamp enrichment and Rust regressions in `crates/utils/src/msg_store.rs`.
- [x] T002 [P] Remove misleading fallback and add rendered regressions in `packages/web-core/src/features/workspace-chat/ui/ConversationTimestamp.tsx` and `.test.tsx`.
- [x] T003 Wire fresh execution constructors in `crates/local-deployment/src/container.rs` (depends on T001).
- [ ] T004 Install locked dependencies; run tests, checks, lint and formatting; document results in `specs/vk/77a2-session-step-tim/validation.md` (depends on T001–T003).
- [x] T005 Independent Codex CLI review; fix confirmed findings and re-verify (depends on T004).
- [x] T006 Update timestamp knowledge topic and index in `docs/knowledge-base/`, commit knowledge (8f518fca) and changes (depends on T005).
- [ ] T007 Open PR, verify checks and merge (depends on T006).
