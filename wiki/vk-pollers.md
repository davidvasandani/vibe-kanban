# VK Pollers: replacing agent-internal background pollers

Every coding-agent CLI ships an in-turn background/watch primitive. Under VK
those primitives are reaped almost immediately, because a turn is one OS process
killed at turn end (see [[agent-process-lifecycle]] for the two kill points).
The agent starts a watcher, the turn ends, the watcher dies, and the execution is
still recorded `completed` / `exit_code 0` — the same silent-abandonment shape as
`ScheduleWakeup` (VAS-283).

A **vk poller** is the replacement: a VK-owned periodic job that outlives the
turn. This page records the parts that are not obvious from the diff.

## A poller is a background helper, deliberately

A poller adds no new persistence axis. It is an
`ExecutionProcessRunReason::BackgroundHelper` whose generated script VK compiled
from a `PollerSpec`, so it inherits the whole substrate unchanged: own process
group (so neither turn-end kill point reaches it), `is_persistent()` ⇒ own raw
log + never-finalize, detach-on-shutdown + re-adopt-by-`pgid` on boot, and
stop-on-archive/teardown.

```
poller  ⟺  run_reason == BackgroundHelper  &&  executor_action.poller.is_some()
```

Consequences to preserve:

- **No migration.** `execution_processes.executor_action` is JSON in a `TEXT`
  column, so `#[serde(default)] poller: Option<PollerSpec>` on `ScriptRequest` is
  backward-compatible; pre-existing rows load with `poller: None`. Adding a
  `ScriptContext` or `run_reason` variant instead would have forced the 6-step
  SQLite CHECK-constraint rebuild for zero behavioural gain.
- **One shared concurrency budget.** Helpers and pollers share
  `MAX_BACKGROUND_HELPERS_PER_WORKSPACE` because they are the same resource. This
  is commented at the constant — it reads like a bug otherwise, and splitting it
  into two caps of five would double the real limit.
- **`PollerSpec` is retained, not re-derived.** The drawer needs command and
  interval; recovering them by parsing the generated shell would be a second
  resolution rule for the same fact.

## Tool-name denial is not sufficient — the chokepoint is a parameter

The important finding, and the one most likely to be re-litigated.

Claude Code 2.1.200's background poll loop is
`Bash(run_in_background: true)` → `Read(<output file path>)`. `TaskOutput` is
*deprecated* in favour of reading that path directly, and `Read` cannot be denied
— it is essential. So denying `Monitor` / `TaskOutput` / `TaskStop` / `Cron*`
removes the ergonomic path and leaves the real one wide open.

The only complete control is a `PreToolUse` hook on `^Bash$` that inspects
`tool_input.run_in_background`, which `--disallowedTools` cannot express because
it is tool-granular. Both layers ship; the hook is the load-bearing one.

Two invariants for anyone touching it:

- The predicate is **conservative**: absent, non-boolean or malformed
  `tool_input` ⇒ allow. An over-broad deny here breaks *every* Claude execution,
  so ambiguity must resolve permissively. This is regression-tested
  ("foreground Bash still allowed in all three modes") and that test is the gate.
- The check sits **before** the `auto_approve` short-circuit, like
  `ScheduleWakeup`, so it fires in bypass/yolo mode — the mode incidents occur in.

The deny reason names `spawn_poller`. A denial that removes a capability without
offering the replacement converts a silent failure into a stuck agent.

## Verify vendor identifiers against the artifact that executes

This task nearly shipped two inert controls. Constitution IX was extended
(0.30.0) to require the discipline.

- **Claude:** `@anthropic-ai/claude-code@2.1.200` on npm is a ~20KB **stub**; the
  CLI is a native binary in `@anthropic-ai/claude-code-linux-x64`. The stub ships
  `sdk-tools.d.ts`, which looks authoritative but lists **JSON-Schema titles, not
  wire tool names** (`FileReadInput` → real tool `Read`). A deny-list built from
  it would match nothing and fail silently. Names must be read from the binary.
  It also contains an alias→canonical map (`BashOutput`/`KillShell` → `TaskOutput`/`TaskStop`),
  and the permission parser normalizes through it, so denying canonical names
  covers aliases.
- **Codex:** `unified_exec` is a *feature flag*, not a tool; the tools are
  `exec_command` and `write_stdin` (an empty `chars` polls without writing). There
  is no per-turn tool allow/deny field, so the lever is
  `features.unified_exec=false` in the thread config. `ConfigToml` has no
  `deny_unknown_fields` and VK does not pass `--strict-config`, so a typo'd key is
  **silently ignored** — the spelling is pinned by a test for that reason.
  Authoritative source is the upstream repo Cargo already vendors for the pinned
  `codex-app-server-protocol` tag, not the npm tarball.
- **Grok:** verified to have **nothing to replace**. ACP defines `terminal/*`
  methods, but VK never advertises the capability (`ClientCapabilities.terminal`
  defaults false, `InitializeRequest::new()` uses the default, the harness never
  mutates it, and the client stubs all five methods `method_not_found`). ACP also
  has no per-tool deny mechanism at all, and `--always-approve` with a nulled
  approval service removes the permission callback under `yolo`. **No rule ships**,
  and the absence is recorded in code as an evidenced decision so it is not later
  "fixed" with a guess. Grok's *own* in-process shell tool name is still unknown;
  VK classifies ACP tool calls by `ToolKind`, never by name, and scrapes the
  command from the display title, so it is near-blind here.

## The generated loop must make failure visible

A poller exists to watch something that is allowed to be broken, so a non-zero
tick is reported and followed by another tick.

The subtle bug: the tick runs in a **subshell** (left-hand side of the
`| head -c` pipeline), so an agent command containing `exit N` terminates that
subshell outright — and any statement placed *after* the call to capture `$?` is
skipped. The status is therefore captured from an `EXIT` **trap** inside the
subshell. Written the obvious way, the loop keeps running but swallows exactly
the failure the poller was created to surface.

Output is capped per tick with an explicit truncation marker: nothing in the
persistent raw-log path rotates or truncates (`process_raw_log_file_path` only
builds a path), and unlike a dev server a poller emits on every tick forever.
Total file growth over uptime remains a pre-existing, shared limitation.

Signals are left at their default disposition so `kill_process_group` still
terminates the loop; only `EXIT` is trapped, for scratch-file cleanup.

## The drawer summary comes from a stream that was already open

Constitution XXVI forbids a collapsed section issuing "a private request solely
to label its header", and requires an existing summary source where one exists.

The tempting implementation — copy `ServerMetricsHeader`, which owns a private
30s `useQuery` — violates it. The next idea, mounting `ExecutionProcessesProvider`
in `WorkspacesLayout`, is *worse than it looks*: `WorkspacesLayout` already calls
`useExecutionProcesses(selectedSession?.id)`, so adding the provider opens a
**second** socket for the same session while appearing to satisfy the principle.

The shipped shape passes `executionProcesses` down as a prop, like `repos` and
`diffs` — so the Pollers section makes **zero** requests, expanded or collapsed.
Every field it needs is already on the streamed `ExecutionProcess`
(`executor_action.typ.poller`, `status`, `started_at`), which is also why no
`api.ts` client was added. A test asserts no `fetch` occurs.

`ExecutionProcessStatus` is a TS **enum**, not a string union — string literals
are not assignable, and statuses must render as themselves (a `killed` poller and
a `failed` poller are different facts; the adopted-process watcher records
`Failed` when a re-adopted group disappears because the real exit code is
unknowable, so `Failed` is not proof of a command failure).

The collapsed header reports the running count **and** signals failure
distinctly, because a bare running count renders "my only poller just died"
identically to "I never had one".

## Not this: a wake-up scheduler

A vk poller runs a **command**, not a turn; it never resumes the agent. Persisting
`(session_id, fire_at, prompt)` and re-invoking via `--resume` +
`QueuedMessageService` is VAS-283 "option B", deliberately deferred alongside
VAS-132. Don't reopen it incidentally — see [[agent-process-lifecycle]].

## Contributed by

- vk/869c-vk-background-po
