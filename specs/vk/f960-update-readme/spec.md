# Feature specification: product README

Feature dir: `specs/vk/f960-update-readme/`. Status: specified.
Command: `/speckit.specify`. The command's checked-in example points to an older task; this task uses its current branch identifier throughout.

## Summary
Give visitors an accurate overview of Vibe Kanban and a practical route to running, developing and operating it. Describe current capabilities together, with ancestry confined to a footnote.

## User stories
- As a prospective user, I want to understand the full issue-to-agent-to-review workflow and automation options.
- As a contributor, I want valid setup commands and links to focused guides.
- As an operator, I want to understand which components and integrations require extra configuration.

## Functional requirements
- FR-1: Present Vibe Kanban as the product, with fork ancestry only in a footnote.
- FR-2: Cover core and added workflows, including planning, executors, isolated workspaces, review/Git, previews, automation, search, integrations and operation.
- FR-3: Explain source installation and development using actual repository scripts and prerequisites.
- FR-4: Distinguish local execution, remote collaboration, optional cluster deployment and single-user authentication behavior.
- FR-5: Link valid guides, repository support and licensing; remove obsolete personal/deployment claims and upstream-only marketing.
- FR-6: Ground all claims in code or maintained docs and record the audit evidence.

## Out of scope
Runtime changes, dependency upgrades, other services, deployment changes and rewriting all detailed docs.

## Acceptance criteria
- [x] Product introduction and one ancestry footnote, no fork-first sections.
- [x] Core and added feature families are covered with source evidence.
- [x] Commands match manifests and build scripts; relative links resolve.
- [x] Optional components and authentication prerequisites are explicit.
- [x] Required formatting and independent review completed, significant findings resolved.
- [ ] Knowledge base updated, PR opened and merged when repository checks permit.

## Open questions
None. User explicitly requests comprehensive coverage, README implementation and PR merge. Reasonable scope is a readable feature guide with links, not a line-by-line code reference.
