# Technical plan (/speckit.plan)

Spec: ./spec.md

## Technical context and approach
Rust services and server crates. Add `has_credit_or_usage_error(&[NormalizedEntry]) -> bool` in `crates/services/src/services/error_remediation.rs`. Inspect full ErrorMessage entries only. Normalize case, apostrophes and whitespace, then match explicit phrases/provider codes. Do not use broad `usage`, `quota`, `429` or `limit` alone. No schema or dependencies required.

In `crates/server/src/error_remediation.rs::handle`, load normalized entries once before `try_reserve`; return with a reason when classified. Extract bounded messages from that same evidence and pass them to `launch`, removing its duplicate read. Existing missing-log fallback remains unchanged. Guard still reserves before any remote call.

## Verification
Table-driven positive/negative tests; ensure earlier and long error entries cannot escape classification through issue-text bounds. Verify non-error entries do not suppress remediation. Run services focused tests, server compilation, required formatting; independent Codex CLI review before wiki/PR.

## Constitution check
II tests contract; III/VI reuse existing consumer and normalizer; IX no vendor schema changes; XI original errors unchanged; XLI no remediation side effects or quota claims for excluded errors. No deviations.

## Risks
Text classification covers recognized phrases, not all possible localized vendor messages. Avoid bare words that suppress genuine code failures. No persisted data model or external contract changes.

Independent review correction: Claude failed Result events were emitted as errors only for AmpResume. Preserve is_error results for both strategies, retain the provider errors array, and exercise the real ClaudeLogProcessor through the admission classifier. Update qa_mock construction for the added field. This changes failed-result diagnostic visibility but leaves ordinary assistant text unclassified.
