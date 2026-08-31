# Spec: VK Pollers — replace agent-internal background pollers

Task: vk/869c-vk-background-po

> Agent Background Pollers regularly die early. For each agent type (Claude,
> Codex, Grok), replace its internal poller with a vk poller. Add the feature to
> the right drawer that lists the poller count when closed and each poller with
> details when expanded.

## Problem

A Vibe Kanban turn is **one OS process**, and it is reaped at turn end by two
independent kill points (`wiki/agent-process-lifecycle.md`):

1. the exit-signal branch — `kill_process_group`, SIGINT→SIGTERM→SIGKILL via
   `killpg` over the whole group; and
2. the monitor tail — `start_kill()` plus `kill_on_drop(true)` when the child is
   removed from `child_store`.

Every coding-agent CLI ships its own in-turn background/polling primitive. Those
primitives spawn children **inside the turn's process group**, so both kill
points reach them. The agent starts a watcher, the turn ends, the watcher is
killed, and the execution is still recorded `completed` / `exit_code 0`. The
failure is silent — which is exactly the pathology VK already fixed once for
Claude's `ScheduleWakeup` (VAS-283) by denying the tool outright rather than
letting it no-op.

This spec generalises that fix to the *background poller* class, and makes the
resulting VK-owned pollers visible.

### What each agent's internal poller actually is (verified)

Identifier strings below were verified against pinned artifacts, not
documentation. This matters: a deny rule built on a wrong string is a control
that silently does nothing, which is worse than no control.

**Claude Code 2.1.200** — verified by extracting the real CLI. The npm package
`@anthropic-ai/claude-code@2.1.200` is a ~20KB stub; the CLI is a native binary in
`@anthropic-ai/claude-code-linux-x64@2.1.200`. The stub's `sdk-tools.d.ts` lists
**JSON-Schema titles, not wire tool names** (`FileReadInput` → real tool `Read`),
so it must not be used as a source for deny rules.

The binary contains an explicit alias→canonical map:

| Role | Canonical | Accepted aliases |
|---|---|---|
| Start background process | `Bash` + `run_in_background: true` | — |
| Read background output | `TaskOutput` | `BashOutput`, `AgentOutput`, `BashOutputTool`, `AgentOutputTool` |
| Kill background process | `TaskStop` | `KillShell`, `KillBash` |
| Long-running watch | `Monitor` | — |
| Timer | `ScheduleWakeup` (already denied) | — |
| Cron | `CronCreate`, `CronDelete`, `CronList` | — |

Two consequences drive the design:

- **Tool-name denial is not sufficient.** `TaskOutput` is deprecated in favour of
  reading the background task's **output file path** with plain `Read`. Denying
  `TaskOutput`/`TaskStop` removes the ergonomic poll but leaves the *spawn* path
  open and `Read` as the poll. `Read` cannot be denied — it is essential.
- **The chokepoint is a parameter, not a tool.** The only complete control is a
  `PreToolUse` hook on `Bash` that inspects `tool_input.run_in_background`.
  `--disallowedTools` is tool-granular and cannot express this.
- `Monitor` is feature-gated in the binary (`isEnabled(){return Hq()&&Pu()}`), so
  it may be absent in some configurations. Denying it is harmless either way.

**Codex 0.144.1** — verified against the upstream repo vendored by Cargo
(`crates/executors/Cargo.toml:41` pins `codex-app-server-protocol` to git tag
`rust-v0.144.1`). `unified_exec` is a **feature flag, not a tool**. The exposed
tools are `exec_command` (returns a `session_id` while the process runs) and
`write_stdin` (takes that `session_id`; **an empty `chars` polls without
writing** — that is the poller). There is no kill tool.

There is **no per-turn tool allow/deny field** on `ThreadStartParams` or
`TurnStartParams`. The only lever is the `config` map:
`features.unified_exec=false` (falls back to one-shot `shell_command`).
`features.unified_exec` defaults to **true on Linux/macOS**.

⚠️ `ConfigToml` has no `serde(deny_unknown_fields)` and VK does not pass
`--strict-config`, so an unknown/typo'd config key is **silently ignored** —
this lever fails open and must be asserted, not assumed.

**Grok** — verified against `agent-client-protocol` 0.8.0 /
`agent-client-protocol-schema` 0.9.1. ACP terminal methods exist
(`terminal/create`, `terminal/output`, `terminal/release`,
`terminal/wait_for_exit`, `terminal/kill`), but **VK never advertises terminal
capability**: `ClientCapabilities.terminal` defaults to `false`,
`InitializeRequest::new()` uses `ClientCapabilities::default()`,
`acp/harness.rs:376-378` never mutates it, and `acp/client.rs:213-246` stubs all
five methods as `method_not_found()`.

**So Grok has no VK-hosted internal poller to replace — that surface is already
closed.** ACP additionally has *no* per-tool deny mechanism at all
(`NewSessionRequest` is `cwd`/`mcp_servers`/`meta`; `PromptRequest` is
`session_id`/`prompt`/`meta`); the only lever is the `session/request_permission`
callback, and `grok.rs:92-94` removes even that when `yolo == true`.

Grok's *own* in-process shell tool name could not be confirmed — no Grok binary,
fixture, or transcript exists in this environment. **This spec therefore ships no
Grok deny rule.** Writing one from a guess would be precisely the inert control
this spec exists to avoid. See "Open questions".

## Goals

1. Agents on Claude and Codex cannot silently start an in-turn background poller
   that will be reaped at turn end.
2. When they need one, they get a **vk poller**: a VK-owned, VK-tracked periodic
   job that outlives the turn and survives a server restart.
3. The right drawer shows the running poller count when collapsed, and each
   poller with details when expanded.

## Non-goals

- **A wake-up scheduler that re-invokes the agent.** `wiki/agent-process-lifecycle.md`
  records this (VAS-283 "option B": persist `(session_id, fire_at, prompt)` and
  re-invoke via `--resume` + `QueuedMessageService`) as *deliberately deferred*, to
  be designed alongside VAS-132. A vk poller runs a **command**, not a turn. It
  does not resume the agent. Reopening that decision is out of scope.
- Changing `ScheduleWakeup` handling — already correct.
- Making `CodingAgent` executions persistent. `is_persistent()` is deliberately
  false for them and asserted in tests; flipping it changes streaming behaviour.
- Grok/ACP tool-name denial (no verified string; see above).

## Design

### The vk poller is a thin, structured layer over background helpers

VK already has the exact substrate a durable poller needs, shipped for dev
servers (PR #55) and extended to background helpers (VAS-111/PR #84):

- spawned into **its own process group** (`group_spawn_no_window`), so neither
  turn-end kill point reaches it;
- `ExecutionProcessRunReason::BackgroundHelper` ⇒ `is_persistent()` ⇒ writes its
  own raw log file (`VK_RAW_LOG_PATH`) and **never finalizes** the execution;
- on server shutdown it is *detached* (`std::mem::forget` defuses
  `kill_on_drop`) rather than killed, and re-adopted next boot by persisted
  `pgid` with a pid-reuse guard;
- already stopped correctly by workspace archive, teardown, and an explicit stop
  route.

Per Constitution VI ("Don't rebuild what shipped") and III ("smallest change"),
a vk poller **reuses all of it unchanged**. What it adds is structure: a poller
is a background helper whose repeating loop VK generates and whose
`(command, interval)` VK retains, so the drawer can show real details instead of
an opaque shell string, and so agents stop hand-rolling daemon loops.

**No DB migration is required.** `execution_processes.executor_action` is a TEXT
column holding `ExecutorAction` JSON. Adding an optional field to `ScriptRequest`
is backward-compatible via `#[serde(default)]`; existing rows deserialize with
`poller: None`. (The generated virtual column indexes `$.type` only, which is
unaffected.)

```rust
pub struct PollerSpec {
    pub command: String,
    pub interval_secs: u32,
}

pub struct ScriptRequest {
    pub script: String,
    pub language: ScriptRequestLanguage,
    pub context: ScriptContext,
    #[serde(default)]
    pub working_dir: Option<String>,
    /// Present when VK generated `script` as a polling loop. Retained so the
    /// poller can be described without re-parsing the generated shell.
    #[serde(default)]
    pub poller: Option<PollerSpec>,
}
```

A poller is therefore *identified* by `run_reason == BackgroundHelper &&
poller.is_some()`. A plain background helper (`poller: None`) keeps working
exactly as today.

### Agent-facing surface

New MCP tools alongside the existing background-helper tools, sharing their
workspace resolution/authorization and their HTTP-proxy shape:

- `spawn_poller { command, interval_secs, working_dir?, workspace_id? }`
- `list_pollers { workspace_id? }` → `{ pollers, count }`
- `stop_poller { execution_process_id }`

VK compiles `(command, interval_secs)` into a bounded loop with a timestamped
per-tick delimiter so the raw log is parseable and each tick is attributable.

The existing per-workspace cap (`MAX_BACKGROUND_HELPERS_PER_WORKSPACE = 5`) and
`working_dir` validation (relative, no `..`) apply unchanged and are **shared**,
not re-derived (Constitution XXI: one convention per concept).

`interval_secs` is validated to a bounded range. A zero/absent interval is
rejected rather than defaulting to a hot loop.

### Closing the internal pollers

**Claude** (`crates/executors/src/executors/claude.rs`) — extend the existing
`ScheduleWakeup` machinery rather than adding a parallel one:

1. Add to `--disallowedTools` in **every** permission mode: `Monitor`,
   `TaskOutput`, `TaskStop`, `CronCreate`, `CronDelete`, `CronList`. The
   permission parser normalizes through the alias map, so canonical names cover
   the aliases; the aliases (`BashOutput`, `KillShell`, `KillBash`,
   `AgentOutput`) are listed anyway for defence-in-depth.
2. Add a `PreToolUse` **deny hook on `Bash`** that denies only when
   `tool_input.run_in_background == true`. This is the load-bearing control. It
   is checked **before** the `auto_approve` short-circuit, so it fires in
   bypass/yolo mode too — the mode the incident occurs in.
3. Catch-all matchers exclude the newly denied names, mirroring the existing
   `^(?!(ExitPlanMode|AskUserQuestion|ScheduleWakeup)$).*` pattern.
4. The deny **reason** names the replacement (`spawn_poller`) and tells the agent
   to keep working, mirroring `SCHEDULE_WAKEUP_DENY_REASON`. A deny that does not
   offer the alternative just converts a silent failure into a stuck agent.

**Codex** (`crates/executors/src/executors/codex.rs`) — set
`features.unified_exec=false` in the `ThreadStartParams` config map. Because
unknown keys fail open, a unit test asserts the exact key string against the
vendored protocol crate rather than trusting the spelling.

**Grok** — no change; the ACP terminal surface is already closed by capability
withholding. This is recorded, with evidence, so a future reader does not read
the absence as an oversight.

### Right drawer

A new `Pollers` section in `packages/web-core/src/pages/workspaces/RightSidebar.tsx`,
following the established `SectionDef` recipe.

- Collapsed: `headerExtra` shows the **running poller count**, and renders
  `null` when the count is zero.
- Expanded: a list — command, interval, status, started-at, most recent tick —
  each with a stop action.
- `fillAvailableSpace: true` (scrollable list body). No `min-h-[Npx]`; the
  `min-h-0` chain is preserved per `wiki/flexible-collapsible-panel-stacks.md`.
- The section is **not** desktop-only: the same `RightSidebar` composition backs
  the mobile `git` tab, and the blast radius is both `local-web` and
  `remote-web`.

**Constitution XXVI compliance is a real constraint here, not a formality.** It
requires the collapsed affordance to take its summary from "an existing
summary/cache source when one exists" and forbids a closed section from issuing
"a private request solely to label its header".

The naive implementation — copying `ServerMetricsHeader`, which owns a private
30s `useQuery` — would violate this. The correct source already exists:
`useExecutionProcesses(sessionId)` is a per-session WebSocket JSON-Patch stream
carrying `run_reason` and `status` for every execution process, and the workspace
chat **already streams it for the same session**. However,
`ExecutionProcessesProvider` is currently mounted only in the carousel column,
the kanban sidebar, and `GitActionsDialog` — **not** in `WorkspacesLayout`.

So the header must not call `useExecutionProcesses` directly (that opens a second
socket for the same session). Instead, `WorkspacesLayout` mounts
`ExecutionProcessesProvider` for the selected session and the header consumes
`useExecutionProcessesContext()`, deriving the count by filtering
`run_reason === 'backgroundhelper' && status === 'running'`. Zero additional
requests; the count keeps updating while the body is unmounted.

Header text needs an explicit truncation boundary and must stay distinguishable
from the section label at the narrowest supported sidebar width (300px).

## Acceptance criteria

Backend
1. `Bash` with `run_in_background: true` is denied for Claude in all three
   permission modes, including bypass, with a reason naming `spawn_poller`.
2. `Monitor`, `TaskOutput`, `TaskStop`, `CronCreate`, `CronDelete`, `CronList`
   appear in `--disallowedTools` in all three modes.
3. Catch-all hook matchers exclude every newly denied tool name.
4. Codex `ThreadStartParams` config contains `features.unified_exec=false`; a
   test pins the key string.
5. `spawn_poller` creates a `BackgroundHelper` execution whose `ScriptRequest`
   carries `PollerSpec`, respects the shared 5-per-workspace cap and
   `working_dir` validation, and rejects an out-of-range `interval_secs`.
6. An `ExecutorAction` JSON written before this change still deserializes
   (`poller: None`) — regression test.
7. A running poller survives turn end and is re-adopted after a server restart
   (exercises the existing persistent path; assert `is_persistent()` and the
   detach/adopt branch are reached).

Frontend
8. Collapsed `Pollers` header shows the running count; shows nothing at zero.
9. The count survives collapsing the section (rendered-DOM test, mirroring
   `RightSidebar.test.tsx`'s existing Git-header test).
10. The header issues no request of its own — it reads
    `useExecutionProcessesContext()`.
11. Expanded body lists each poller with command, interval, status, started-at,
    and a stop action.
12. Section root classes follow the `fillAvailableSpace` contract; no fixed
    height minimum.

## Rejected alternatives

- **Keep the agents' internal pollers and try to keep the turn process alive.**
  Rejected: contradicts the one-turn-one-process identity chain, requires
  defeating both kill points, and `CodingAgent` is deliberately non-persistent.
- **Deny Claude's poller tools by name only.** Rejected: leaves
  `Bash(run_in_background)` open with `Read`-on-output-file as the poll — a
  control that looks complete and is not.
- **Build `PollerSpec` as a new `ScriptContext` / `run_reason` variant.**
  Rejected: needs a SQLite CHECK-constraint migration and a new persistence axis
  for no behavioural gain, since the poller wants byte-for-byte the background
  helper's lifetime.
- **Reuse `spawn_background_helper` as-is and label it "Pollers" in the UI.**
  Rejected: the drawer would show an opaque shell string as "details", and every
  agent would hand-roll its own loop/sleep/exit semantics — the fragility being
  removed.
- **Give the header its own polled query (the `ServerMetricsHeader` shape).**
  Rejected: violates Constitution XXVI and opens a duplicate socket for a session
  already being streamed.
- **Set `features.shell_tool=false` for Codex.** Rejected: disables the shell
  entirely, breaking ordinary command execution.

## Risks and residual dependencies

- **Claude tool names are pinned to 2.1.200.** They were read from that exact
  binary. A Renovate bump of `@anthropic-ai/claude-code` could rename or add
  tools, silently reopening the gap. Note that this package is already a
  `needs-review` Renovate carve-out, so a human sees the release notes; the deny
  list should be called out in that review.
- **The same harness-tool assumption as VAS-283**: both layers assume the CLI
  subjects these tools to `--disallowedTools`/PreToolUse hooks like normal
  registry tools. `ScheduleWakeup` demonstrates it holds today. The
  CLI-independent fallback remains detecting the `ToolUse` during log
  normalization.
- **Codex config fails open.** An unknown key is silently ignored. The unit test
  pins the string, but only a live run proves adoption.
- **Grok is unaddressed by design.** ACP terminals are closed, but Grok's own
  in-process shell may still background work, and VK is nearly blind to it: tool
  calls are classified by `ToolKind` (`execute`), never by name, and
  `parse_execute_command` scrapes the command from the display *title*.

## Open questions

1. **Grok's internal shell tool name** — needed before any Grok-side rule. Should
   be captured from a real `session/update` ToolCall, not guessed.
2. **Should a poller be stopped when its workspace's last session ends?** Today a
   background helper lives until stopped, archived, or torn down. Pollers inherit
   that. Confirm this is wanted rather than an interval-bounded lifetime.
3. **Tick retention.** The raw log grows with uptime. Constitution XIX requires a
   bounded window for live streams; confirm whether the poller loop should
   self-truncate or rely on existing raw-log handling.

## Incidental finding (not in scope)

`codex.rs:568` sets `include_apply_patch_tool`, which **does not exist** in
`codex` 0.144.1 (zero matches in the vendored upstream). It is silently swallowed
— dead config today. Worth a separate issue; deliberately not changed here.
