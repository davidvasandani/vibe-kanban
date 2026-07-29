# Implementation Plan

1. Inspect the composed-pipeline contract, bundled asset seeding behavior, and
   focused tests to define the smallest compatible change.
2. Refresh the `Parallel Sub-Agents` prompts so fan-out is genuinely
   concurrent, every provider receives the task before execution, read tools
   remain available under a read-only policy, failures are isolated, and later
   rounds start fresh children with both original and synthesized context.
3. Extend bundled seeding with a narrowly scoped legacy-content refresh:
   replace the on-disk parallel pipeline only when it byte-for-byte matches the
   previously shipped default, preserving every customized copy.
4. Add regression tests for the prompt contract and for both unmodified-default
   upgrade and customized-file preservation.
5. Run formatting and focused Rust tests, then the broader checks warranted by
   the touched backend module.
6. Run an independent Codex diff review, address confirmed findings, and repeat
   verification until no significant findings remain.
7. Record reusable bundle-refresh and prompt-orchestration knowledge in the
   project knowledge base, update its index, and commit the knowledge-base
   changes as required by the task pipeline.
