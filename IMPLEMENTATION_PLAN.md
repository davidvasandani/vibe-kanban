# Implementation plan — vk/88c5-marking-an-issue

1. Read prior knowledge at workspace `PRIOR_KNOWLEDGE.md`; trace remote terminal-status archival through ProjectProvider reconciliation to local workspace persistence and drawer subscriptions.
2. Execute the task-specific workspace SpecKit commands in order, storing artifacts at their specified homelab path. Reaffirm service and hosting constitutions; document the concrete missing boundary and acceptance cases.
3. Extend the existing reconciliation/archive mechanism at that boundary. Preserve authorization, identity, transactional remote writes, archive-only semantics, and independent workspace failures.
4. Add focused regressions for the observed failure, persisted archive propagation, repeated updates, unrelated workspaces, and retries. Install locked dependencies before verification and run required formatting plus relevant checks.
5. Run independent Codex CLI diff review, fix confirmed findings, and re-verify.
6. Update and commit reusable service knowledge tagged with this task. Commit artifacts and code, open pull request(s) against their base branches, and merge after required checks.
