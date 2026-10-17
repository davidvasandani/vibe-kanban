# Technical plan

Spec: ./spec.md. Status: ready for tasks.

## Architecture
- Add `packages/web-core/src/shared/lib/issueWorkspaceAdvisory.ts`: pure projection of project workspace rows, workspace-scoped PRs and local sidebar metadata, scoped by project/issue UUID. Stable sorting and identity-based dismissal signature.
- Add `packages/ui/src/components/IssueWorkspaceWarning.tsx`: presentational accessible advisory, workspace list, branch/status, open PR emphasis, dismiss and open actions; localized using common namespace.
- Add `packages/web-core/src/shared/components/IssueWorkspaceWarningContainer.tsx`: subscribe to existing PROJECT_WORKSPACES_SHAPE and PROJECT_PULL_REQUESTS_SHAPE for the linked project, enrich from WorkspaceContext, scope dismissal by issue + sibling identity, navigate using existing app navigation and ownership rules.
- Render this container in `CreateChatBoxContainer.tsx` when linkedIssue exists, above both repo and prompt steps. The existing create mutation and canSubmit remain unchanged.
- Extend `packages/ui/src/components/IssueWorkspacesSection.tsx` headerExtra with plural non-archived count. Preserve collapsed visibility and existing section behavior.
- Add common strings to all supported locales to satisfy the repository translation-key consistency check.

## Verification
Use remote-web Vitest/jsdom for projection and container/UI tests, including navigation, dismissal, new sibling, absent metadata, archived and other-issue exclusions and collapsed badge. Run frontend type checks, lint and repository formatting. Rust remains unchanged; no generated contracts or new dependencies.

## Constitution check
II: meaningful contract tests. III/VI: reuse data, navigation, creation flow. IV: presentation in UI, subscriptions/state in web-core. XXXV: exact workspace/project/issue identity for all joins. No deviations.

## Risks
Synced metadata is eventually consistent, so simultaneous starts may precede discovery. Remote workspace schema lacks branch; explicitly label missing enrichment. Open only IDs owned by the signed-in user with a matching current-host workspace. Ownership alone cannot establish host-safe navigation. No claim that file counts prove unmerged commits.
