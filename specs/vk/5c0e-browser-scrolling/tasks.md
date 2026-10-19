# Tasks
Applied `/speckit.tasks`. Sequential implementation, with independent validation in parallel.

- [x] T001 Add typed Scroll and CDP dispatch in crates/services/src/services/browser/{types,cdp}.rs; regenerate shared/types.ts.
- [x] T002 Add wheel/swipe handling in packages/web-core/src/pages/workspaces/{BrowserPanelContainer.tsx,useBrowserScrollInput.ts} (depends T001).
- [x] T003 [P] Add frontend regression coverage in packages/web-core/src/pages/workspaces/BrowserPanelContainer.test.tsx (depends T002).
- [x] T004 [P] Add Rust protocol/control/live scrolling coverage in crates/services/src/services/browser/{mod,tests,cdp}.rs (depends T001).
- [x] T005 Format and verify frontend and Rust checks, independent Codex review; fix findings (depends T003,T004).
- [x] T006 Record reusable knowledge in wiki/browser-session-control-arbiter.md and wiki/INDEX.md, commit (depends T005).
- [x] T007 Open PR #377 against main with required CI as the merge gate (depends T006). Final merge state is tracked in GitHub: https://github.com/davidvasandani/vibe-kanban/pull/377.
