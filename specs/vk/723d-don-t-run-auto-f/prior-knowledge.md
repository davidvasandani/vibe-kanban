# Prior knowledge

Read-only search: `wiki/*.md` for credit, quota, usage limit, rate limit and remediation.

- `wiki/auto-error-remediation.md`: finalization emits failed-turn events without I/O; server consumer owns admission and remote side effects. Preserve recursion, dedupe, hourly cap and similarity reuse. Reserve before remote mutations, but classify quota evidence before reservation.
- `wiki/agent-process-lifecycle.md` (as referenced by remediation knowledge): terminal statuses have distinct semantics; leave Failed/Killed/Interrupted/Indeterminate trigger behavior intact.
- No existing page defines credit/usage failure exclusion. Provider error types and normalizers must supply the implementation evidence.

Task: 723d-don-t-run-auto-f. Knowledge base unchanged during recall.
