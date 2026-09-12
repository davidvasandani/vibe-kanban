# Clarifications

## Decisions

### What counts as a completed round?

A round completes after all successfully launched children have returned a final
response and provider launch/authentication failures have been recorded. A
failed launch is not itself a round, and the orchestrator does not repeatedly
spend the round budget on that failure. If no provider launches successfully,
the pipeline reports that the fan-out could not run rather than synthesizing a
fictional round.

### Does "exact same prompt" forbid later-round context?

The original task prompt must remain intact and identifiable in every child's
initial input. On rounds after the first, the previous synthesis is appended as
clearly separated context. The synthesis does not rewrite or replace the
original task.

### How is read-only behavior enforced without disabling tools?

The pipeline contract tells child agents to analyze without editing and directs
the orchestrator to choose a read-only sandbox/permission mode where its launch
interface supports one. It must not pass an empty/disabled tool set, because
repository inspection is essential to a grounded answer. Provider-specific flag
spelling stays outside the bundled prompt so the workflow remains compatible
with supported CLI evolution.

### How should an already seeded default be upgraded?

Use a narrow, content-addressed migration in seed reconciliation. The code
recognizes the exact bytes of the one previously shipped
`parallel-subagents.toml`; only that content is replaced with the new embedded
default. Any difference is treated as user ownership and preserved. An already
known but deleted file remains deleted.

### Should provider failures trigger substitute agents?

No. The named-provider comparison is the product contract. Successful outputs
are synthesized and missing providers are identified concisely; the orchestrator
must not impersonate the provider, embed its own response as a substitute, or
launch serial replacements.

## Open Questions

None.
