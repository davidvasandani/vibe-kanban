# Tasks

Plan: ./plan.md. Independent files within each [P] layer may be edited together.

## Layer 1
- [x] T001 [P] Add projection and signature in `packages/web-core/src/shared/lib/issueWorkspaceAdvisory.ts`.
- [x] T002 [P] Add presentation in `packages/ui/src/components/IssueWorkspaceWarning.tsx`, plural header in `packages/ui/src/components/IssueWorkspacesSection.tsx`, and strings in `packages/web-core/src/i18n/locales/*/common.json`.

## Layer 2
- [x] T003 Wire subscription/dismissal/navigation in `packages/web-core/src/shared/components/IssueWorkspaceWarningContainer.tsx` and `packages/web-core/src/shared/components/CreateChatBoxContainer.tsx` (T001/T002).

## Layer 3
- [x] T004 Add projection and rendered UI/container regression tests in `packages/remote-web/src/test/IssueWorkspaceWarning.test.tsx` (T003). Run install, frontend checks/lint, formatting and tests.

## Layer 4
- [x] T005 Independent Codex review; fix confirmed findings and reverify (T004).
- [x] T006 Document reusable behavior in `wiki/issue-workspace-advisory.md`, update `wiki/INDEX.md`, commit (T005).
- [ ] T007 Open task PR against main and merge after checks (T006).
