# Implementation plan
Task: vk/e464-issue-and-worksp

1. Refresh SpecKit constitution applicability and feature artifacts for this task, resolving comment semantics from existing request paths.
2. Trace comment acceptance, queue acceptance, workspace archive mutations and remote issue synchronization.
3. Unarchive upon accepted workspace comments using the established lifecycle service.
4. Extend remote workspace mutation transaction to reopen only linked Done issues on archived-to-active transitions.
5. Add regression coverage for transitions, unchanged statuses, unlinked workspaces, and accepted comment paths.
6. Run locked dependency setup, required formatting, and relevant checks/tests.
7. Run independent Codex diff review; fix confirmed findings and reverify.
8. Commit reusable knowledge with task tag, create PR against base, and merge after verification.
