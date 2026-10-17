# Feature specification: duplicate issue workspace advisory

Status: specified. Feature dir: `specs/vk/fe2d-warn-when-starti/`.

## Summary and user stories
As an issue owner starting work, I want to see existing active workspaces before launching another agent so I can reuse work already underway. As a reviewer, I want a visible count on the issue so accidental duplicate runs are discoverable.

## Functional requirements
- FR-1: Identify siblings by exact linked issue identity and exclude archived workspaces.
- FR-2: During workspace preparation show a dismissible warning with each sibling's name, branch and known activity status. Missing metadata is explicitly unknown.
- FR-3: Offer to open accessible existing workspaces. Keep starting another workspace available without mandatory confirmation, even if metadata cannot load.
- FR-4: Emphasize workspace-specific open PR or available change evidence; never label unknown state as clean or claim file counts prove unmerged commits.
- FR-5: More than one active workspace produces an active count on the issue, visible even if its workspace section is collapsed.
- FR-6: Live sibling additions restore a dismissed warning; switching issues resets dismissal.

## Acceptance criteria
- Zero active siblings or archived-only siblings show no warning.
- One or more active siblings show identifying information; unrelated same-title records are ignored.
- Dismiss removes the advisory, open navigates to the identified workspace, and neither action becomes a prerequisite for creating.
- A PR on sibling A does not appear as evidence on sibling B; absent branch metadata is labeled unavailable.
- Two active siblings show “2 active workspaces” in the issue header; archiving one removes that plural signal.

## Out of scope
Title matching, backend creation locks, atomic concurrent-start prevention, cross-agent mid-run notifications, pre-merge overlap detection and changes to other services.

## Open questions
None; clarification records the metadata and navigation policy below.

## Clarifications
- Active means non-archived, irrespective of running/idle/completed agent state or merged PRs.
- Shared project records include other owners. Warn about these too; offer open only for owned workspaces present on the current host; ownership alone does not establish host availability.
- Project workspace records do not contain branch names. Enrich from matching local workspace records; remote-only records still warn with “Branch unavailable”. No invented branch from a title or PR target.
- Use available open-PR evidence for strong emphasis. File-diff counts may indicate changes but are not proof of unmerged commits; no new expensive per-sibling Git polling.
- This is an advisory based on asynchronously available data, not an atomic guard against two simultaneous starts.
