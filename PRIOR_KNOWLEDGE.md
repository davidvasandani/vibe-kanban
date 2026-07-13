# Prior Knowledge — recalled for `vk/c59f-default-to-origi`

Searched the project knowledge base (`wiki/` — 11 topic pages + INDEX) for
pages relevant to this task: defaulting a repository's target branch to
`origin/main` on the create-issue repo-picker screen
(`CreateModeRepoPickerBar.tsx`, create-mode state, `repoApi.getBranches`).

## Result: no directly on-topic page

No existing page covers the create-mode repo/branch picker, target-branch
defaulting, or the `create-mode` state machine. The `branch` hits across the
wiki are all incidental — git branches of *code* or executor turn-completion
branches, not repository branch selection. So this task builds on the code, not
on recorded knowledge.

## Tangentially related pages (constraints noted, not reused wholesale)

- **[kanban-issue-panel-sections.md]** — the *other* create/edit panel
  (`KanbanIssuePanel.tsx`, the local-kanban issue detail/create panel). Distinct
  component from the create-mode chat screen this task touches
  (`CreateChatBoxContainer` → `CreateModeRepoPickerBar`). Useful only as a
  reminder that VK has more than one "create" surface; the change here is
  scoped to the create-mode chat flow and does not touch `KanbanIssuePanel`.
- **[task-pipeline-block.md]** — establishes the repo's convention of small,
  well-tested, uncontrolled-seeding UI logic with round-trip contracts. The
  approach here mirrors that ethos: a pure, unit-tested resolver rather than
  branch-defaulting logic scattered through the component.

## Constraints carried into design (from the constitution, not the wiki)

- VK fork must **stay mergeable with upstream**: prefer additive files, keep the
  one edited upstream file's diff minimal and local. → New `defaultBranch.ts`
  helper + a localized edit to `CreateModeRepoPickerBar.addRepoWithBranchSelection`.
- **Generated artifacts have one source**: this task deliberately avoids any
  Rust/type change so no `generate-types` / `prepare-db` regeneration is needed.

If reusable knowledge emerges from this task, it will seed a new wiki page in
stage 12 (there is currently no page to extend).
