# Feature specification: no auto-fix for exhausted credits or usage

Feature dir: specs/vk/723d-don-t-run-auto-f/
Status: specified via /speckit.specify

## Summary and user story
As a user whose agent has exhausted credits or usage allowance, I want Vibe Kanban to preserve the error without starting an auto-fix agent, because code changes cannot replenish that allowance.

## Functional requirements
- FR1: Recognized exhausted-credit, billing-quota and usage/rate-limit failures do not create auto-fix issues, recurrence comments or workspaces.
- FR2: Such failures do not consume per-workspace dedupe or hourly remediation capacity.
- FR3: Recognize explicit errors regardless of case and before diagnostic truncation; ordinary uses of words like usage, credit, limit or quota are insufficient evidence.
- FR4: Ordinary code failures retain the current remediation policy, including opt-in, recursion, dedupe and similarity guards. User-visible errors remain unchanged.

## Acceptance criteria
- Positive regression fixtures cover exhausted credits, insufficient quota, usage limit and rate-limit messages.
- Negative fixtures cover code failures, CLI usage syntax, context limits and quoted quota wording in non-error conversation entries.
- Admission happens before guard reservation and all remote remediation operations.
- Empty evidence preserves existing behavior.

## Out of scope
Purchasing credits, retry scheduling, changing executors, schema/config/UI changes, and unrelated services.

## Open questions
Resolved by /speckit.clarify: usage includes explicit provider rate-limit failures; like depleted credits, these cannot be fixed by editing the repository. CLI syntax errors and context-window exhaustion are not usage allowance failures. No open questions.
