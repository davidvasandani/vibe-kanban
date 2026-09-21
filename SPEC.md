# Bounded waiting inside an agent turn

Prevent a Vibe Kanban turn from hanging on a wait that will never finish. An
agent reached for the `Monitor` watch tool, was denied by an existing VK
control, fell through to a plain foreground `until grep -q …; do sleep 5; done`
loop, and blocked the turn for about an hour. Both existing controls worked: the
`Monitor` tool-name denial fired, and the `run_in_background` parameter denial
correctly did not, because nothing was backgrounded. The gap is that they bound
two paths to an unbounded wait while the effect stayed reachable by a third.

Two layers, both in `crates/executors`, Claude executor only.

**Layer 1 — a VK-owned command bound.** VK states `BASH_DEFAULT_TIMEOUT_MS`
(120000) and `BASH_MAX_TIMEOUT_MS` (600000) explicitly on the Claude child
process instead of inheriting the CLI's defaults. The values equal the pinned
CLI's current ones on purpose: the deliverable is ownership and
bump-resistance, not tightening — `cargo test --workspace` and
`pnpm install --frozen-lockfile` legitimately exceed five minutes, so a shorter
cap would break real work. Seeded before the execution environment is applied,
so an operator or organisation variable of the same name still wins. Reaching
the bound returns control to the turn; the CLI may detach rather than kill the
process, and VK's existing turn-end process-group kill reaps anything left.

**Layer 2 — refuse unbounded foreground waits.** The existing `^Bash$`
`PreToolUse` chokepoint gains a second predicate: deny when a command has a
`while`/`until` keyword **and** a `sleep` **and** no bounding marker (`timeout`,
`SECONDS`, `-lt`, `-le`, `-gt`, `-ge`), or leads with `watch`. `for` loops are
never denied. The refusal names `spawn_poller` and its mandatory stop rules, so
the agent redirects rather than stalls. No new callback id, matcher, hook
registration, config surface, or dependency.

The predicate is deliberately conservative: absent, non-string or unrecognised
commands are allowed. `Bash` is the workhorse tool, so an over-broad deny would
be a worse regression than the bug it fixes.

Acceptance: the incident's command is refused with a `spawn_poller` redirect
delivered over the real protocol; ordinary long commands, retry loops with a
counter, `timeout`-wrapped waits and `while read` loops are not refused; an
operator override beats VK's default; a maximum below the default fails the
build rather than becoming silently inert; every pre-existing foreground and
background `Bash` guard still passes.

Scope: Claude only. Codex has no `PreToolUse` equivalent and its shell-deadline
identifier was not verified against the pinned artifact, so nothing ships for it
— recorded as a deferral with evidence, not an oversight. Grok's existing
verified-absence record is unchanged. No turn-level supervisor, no change to
`spawn_poller`, no deployment or hosting changes.

Full artifacts: `specs/vk/603d-prevent-stuck-jo/` in the homelab repository
(`spec.md`, `research.md`, `plan.md`, `tasks.md`, `validation.md`).
