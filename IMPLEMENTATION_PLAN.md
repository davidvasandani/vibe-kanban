# Implementation plan — bounded waiting inside a turn

Full artifacts in `specs/vk/603d-prevent-stuck-jo/` (homelab repo). Steps as
executed, in order.

1. Verify the enforcement identifiers against the **pinned artifact**, not
   documentation: read `BASH_DEFAULT_TIMEOUT_MS` / `BASH_MAX_TIMEOUT_MS` and
   their `max(requested, effective_default)` clamp out of the
   `@anthropic-ai/claude-code@2.1.268` native binary. Record the extract and the
   version in `research.md` (Constitution IX).
2. Add `BASH_DEFAULT_TIMEOUT_MS` (120_000), `BASH_MAX_TIMEOUT_MS` (600_000) and
   a testable `bash_timeout_env()` to `crates/executors/src/executors/claude.rs`,
   with the verification source in the doc comment. Guard `max >= default` with
   a `const` assertion so an inert control breaks the build rather than passing
   quietly.
3. Seed those variables in `ClaudeCode::spawn_internal` **before**
   `apply_to_command`, so a profile/org/operator variable of the same name still
   overrides them.
4. Add `UNBOUNDED_WAIT_DENY_REASON`, a token-boundary helper, and
   `is_unbounded_wait_command()` to
   `crates/executors/src/executors/claude/client.rs`. Deny on `while`/`until`
   **and** `sleep` **and** no bounding marker, or a leading `watch`; never deny
   `for`; allow anything malformed or unrecognised.
5. Extend the existing `DENY_BACKGROUND_BASH_CALLBACK_ID` dispatch arm to try
   the background predicate first, then the wait predicate, preserving the
   foreground fall-through. No new callback id, matcher or hook registration —
   the rule inherits bypass/yolo coverage automatically.
6. Record the Claude-only scope in code: Codex has no `PreToolUse` equivalent
   and its shell-deadline identifier is unverified; Grok's absence record
   stands. An unverified identifier must not ship, and the absence must read as
   a decision.
7. Test: predicate allow/deny cases including the incident command and the
   retry-loop false positive; malformed input; word boundaries; the deny
   response's content; denial in bypass mode; precedence of both predicates;
   env-var spelling, serialization and override precedence via
   `as_std().get_envs()`; and a **duplex protocol** case proving the refusal is
   delivered rather than merely constructed.
8. Confirm the pre-existing foreground/background `Bash` guards still pass
   unchanged — they are the gate against an over-broad deny.
9. Verify the `const` guard empirically by temporarily lowering the maximum and
   observing the build fail, then restore.
10. Run `pnpm install --frozen-lockfile`, `cargo test --workspace`,
    `pnpm run format`, `pnpm run backend:check`, `pnpm run lint`.
11. Independent Codex review of the diff; address confirmed findings and
    re-verify.
12. Extend `wiki/vk-pollers.md` with the displacement finding and the clamp,
    refresh `wiki/INDEX.md`, then open and merge the pull request.
