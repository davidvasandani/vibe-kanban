# Feature Specification: Reliable Parallel Sub-Agent Pipeline

**Feature dir**: `specs/vk/a17f-fix-parallel-age/`
**Status**: Draft
**Task**: `vk/a17f-fix-parallel-age`

## Summary

Make the bundled Parallel Sub-Agents workflow reliably obtain independent,
complete analyses from Claude, Codex, and Grok. The workflow must launch
providers concurrently, deliver the task before taking away any capability,
retain workspace-reading tools under a non-mutating policy, isolate provider
failures, and carry the original prompt plus the prior synthesis into each
bounded follow-up round.

Existing users whose on-disk file is still the exact previously shipped default
must receive the correction automatically. User-customized or deleted copies
must remain untouched.

## Why

The current natural-language launch contract is ambiguous. An orchestrator may
attempt to make children safe by disabling their tools, leaving Codex unable to
read the prompt or workspace; it may retry unavailable providers serially and
consume the round budget; and it may substitute its own synthesis for missing
child output. Users then receive operational error narration and a partial
answer instead of the promised multi-provider result.

## User Stories

- As a pipeline user, I want all available providers to analyze the same task
  concurrently so that the result contains genuinely independent perspectives.
- As a child-agent user, I want each provider to retain read access to relevant
  workspace context so that its answer is grounded rather than declined or
  partial.
- As a user with one unavailable provider, I want the other providers to finish
  without serial retry delays or fabricated substitute responses.
- As a user who customized a bundled pipeline, I want application updates to
  preserve my local workflow.

## Functional Requirements

- FR-1: The fan-out contract MUST start Claude, Codex, and Grok concurrently
  through available agent or noninteractive CLI interfaces.
- FR-2: Each child MUST receive the exact original task prompt as its initial
  task input before follow-up context or termination.
- FR-3: Each child MUST retain tools needed to inspect the workspace. The
  orchestrator MUST use read-only instructions and permission/sandbox policy
  instead of disabling all tools.
- FR-4: The orchestrator MUST wait for and label every successful provider's
  complete output before synthesis.
- FR-5: Launch, authentication, and provider availability failures MUST be
  isolated and reported by provider without blocking successful children.
- FR-6: Failed launch attempts MUST NOT count as completed analysis rounds.
- FR-7: Every later round MUST use fresh concurrent children and include both
  the exact original task prompt and the preceding synthesis.
- FR-8: The workflow MUST run at most operator-selected `N` completed rounds,
  defaulting to three, and MAY stop early when substantive results converge.
- FR-9: The orchestrator MUST NOT invent a child response or embed its own
  synthesis as a replacement for a failed provider.
- FR-10: An on-disk parallel pipeline that byte-for-byte matches the prior
  bundled default MUST be refreshed to the corrected default.
- FR-11: Customized, deleted, or unrecognized on-disk pipeline content MUST NOT
  be overwritten or resurrected by automatic refresh.

## Non-Functional Requirements

- NF-1: Keep the existing pipeline TOML schema, stage IDs, stage ordering, and
  default-enabled behavior.
- NF-2: Do not add a backend provider orchestration service.
- NF-3: Regression tests must cover semantic safety/completion clauses, not only
  provider-name keywords.
- NF-4: Seed refresh must be deterministic, idempotent, and fail-safe toward
  preserving user content.

## Out of Scope

- Installing or authenticating external providers.
- Pinning provider models for the fan-out.
- Letting child analysts modify repository files.
- Changing the task-create pipeline UI or API schema.

## Acceptance Criteria

- [ ] Bundled prompt tests require concurrency, exact initial prompt delivery,
      retained workspace-reading tools, and an explicit prohibition on
      disabling all tools.
- [ ] Follow-up prompt tests require fresh concurrent children, original prompt
      plus prior synthesis, bounded rounds, and failure isolation.
- [ ] Focused tests prove the exact legacy default upgrades.
- [ ] Focused tests prove a one-byte-customized file and a deleted known file
      remain unchanged.
- [ ] The bundled pipeline parses with the existing four stage IDs.
- [ ] Repository formatting and focused Rust tests pass.
- [ ] Independent Codex review finds no significant unresolved issue.

## Open Questions

See `clarifications.md`.
