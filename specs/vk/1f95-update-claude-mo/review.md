# Independent Codex Review

The requested `codex-review` skill was not installed, so the independent review
was run with the Codex CLI in an ephemeral read-only session over the complete
uncommitted diff.

## Iteration 1

The generic `codex review --uncommitted` command reported that embedded Claude
pins are normally Renovate-owned. The task's explicit instruction to update the
Claude Code dependency was not available to that review invocation. A second
review was run with the exact task authorization and the repository's required
manual alias/native-artifact safeguards; no code or policy was changed merely
to suppress the observation.

## Iteration 2

Significant finding: `claude-fable-5-1` was added to the catalog but still
received the 200K fallback from `context_window_for_model()`, despite the native
catalog declaring a native 1M window.

Resolution: classify Fable 5.1 as native 1M, add focused coverage, update the
specification/research/contracts, rerun all 41 Claude tests and executor check.

## Iteration 3

Significant finding: the explicit Fable fix left its retained `fable` alias on
the 200K fallback and exposed the same pre-existing inconsistency for current
Opus/Sonnet aliases and Sonnet 5.

Resolution: classify all current native-1M choices consistently (`opus`,
`sonnet`, `fable`, Opus 5, Sonnet 5, Fable 5.1), retain `[1m]` suffix handling,
and test each case. All 41 Claude tests, `cargo check -p executors`, formatting,
and diff checks passed again.

## Final review

`No significant findings.`
