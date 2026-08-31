# Implementation Plan: VK Pollers (vk/869c-vk-background-po)

Ordered so each layer is independently reviewable and the risky, verifiable
pieces land before the cosmetic ones. See `SPEC.md` for rationale and the
verified identifier strings.

## Layer 0 — Prerequisite

0.1 `pnpm install --frozen-lockfile` (fresh-worktree requirement, per
`CLAUDE.md` / Constitution XIV).

## Layer 1 — `PollerSpec` on the executor action (no migration)

1.1 `crates/executors/src/actions/script.rs`
- Add `PollerSpec { command: String, interval_secs: u32 }` deriving
  `Debug, Clone, Serialize, Deserialize, PartialEq, TS`.
- Add `#[serde(default)] pub poller: Option<PollerSpec>` to `ScriptRequest`.
- **Do not** add a `ScriptContext` variant — pollers reuse `BackgroundHelper`
  (see SPEC "Rejected alternatives"); adding one forces a SQLite CHECK-constraint
  migration for zero behavioural gain.
- Fix every existing `ScriptRequest { .. }` literal in the workspace (struct
  literals are exhaustive). Expect hits in `crates/server/src/routes/workspaces/execution.rs`,
  `crates/db/tests/migrations.rs`, and container/service tests.

1.2 Loop compiler — `fn compile_poller_script(&PollerSpec) -> String`, colocated
with `PollerSpec`.
- Emits a bounded `while` loop with a timestamped per-tick delimiter so the raw
  log is attributable per tick.
- `set -u`; the tick body must not abort the loop on non-zero exit (a poller
  whose command fails once must keep polling) — but it must propagate a
  terminating signal so `kill_process_group` still works.
- Interval is applied via `sleep`; the command is interpolated as a shell body,
  not `eval`'d on a quoted string.

1.3 Validation — `MIN_POLLER_INTERVAL_SECS` / `MAX_POLLER_INTERVAL_SECS`
constants. Reject `0` explicitly rather than defaulting (a silent default here is
a hot loop).

1.4 Tests (`#[cfg(test)]`, same file)
- Round-trip `ScriptRequest` with and without `poller`.
- **Backward-compat regression**: deserialize a `ScriptRequest` JSON literal with
  no `poller` key → `poller: None`. This is the acceptance criterion that proves
  no migration is needed.
- `compile_poller_script` contains the command, the interval, and the delimiter.
- Interval bounds: `0` and over-max rejected; min/max accepted.

## Layer 2 — Server routes

2.1 `crates/server/src/routes/workspaces/execution.rs`
- `POST /background-helpers/start` currently builds the `ScriptRequest`. Add
  `POST /pollers/start` and `GET /pollers` **sharing** the existing cap check
  (`MAX_BACKGROUND_HELPERS_PER_WORKSPACE`) and `is_valid_helper_working_dir` —
  extract the shared preamble into one helper rather than duplicating it
  (Constitution XXI: one convention per concept, one resolver).
- The cap is shared across helpers **and** pollers (they are the same resource);
  say so in a comment so a later reader does not "fix" it into two caps.
- `StartPollerError` extends the existing error enum shape with
  `InvalidInterval`. Error messages name what failed and for which entity
  (Constitution XXI).
- `GET /pollers` filters `run_reason == BackgroundHelper` **and**
  `poller.is_some()`, returning `{ pollers, count }`.
- Analytics: `poller_started`, mirroring `background_helper_started`.

2.2 `crates/server/src/bin/generate_types.rs` — export `PollerSpec` and the new
request/response types; then `pnpm run generate-types`. Never hand-edit
`shared/types.ts`.

2.3 Tests — interval validation and the poller/helper discrimination filter.

## Layer 3 — MCP tools

3.1 `crates/mcp/src/task_server/tools/pollers.rs` — modelled directly on
`background_helpers.rs` (thin HTTP proxies):
- `spawn_poller`, `list_pollers`, `stop_poller`.
- Reuse `resolve_workspace_id` + `scope_allows_workspace` exactly as the helper
  tools do.
- **Note the existing asymmetry**: `stop_background_helper` performs *no*
  workspace-scope check. Match the sibling's behaviour for consistency, but flag
  it in the PR — do not silently "fix" one and leave the other, and do not
  silently copy a gap without naming it.
- Tool descriptions must state the durability contract ("survives the end of the
  current agent turn") — that sentence is what makes the agent choose it.

3.2 Register in `crates/mcp/src/task_server/tools/mod.rs` for both
`global_mode_router` and `orchestrator_mode_router`; extend the orchestrator
allowlist test with the three new names.

3.3 `crates/mcp/AGENTS.md` — document pollers next to background helpers.

## Layer 4 — Close Claude's internal poller

`crates/executors/src/executors/claude.rs` (+ `claude/client.rs`)

4.1 Constants next to `SCHEDULE_WAKEUP_TOOL`:
- `BACKGROUND_POLLER_TOOLS` = canonical `Monitor`, `TaskOutput`, `TaskStop`,
  `CronCreate`, `CronDelete`, `CronList`, plus aliases `BashOutput`,
  `BashOutputTool`, `AgentOutput`, `AgentOutputTool`, `KillShell`, `KillBash`.
  Comment: canonical names normalize through the CLI's alias map; aliases are
  listed for defence-in-depth, and the list is pinned to
  `@anthropic-ai/claude-code@2.1.200`.
- `DENY_BACKGROUND_BASH_CALLBACK_ID`, and a reason string naming `spawn_poller`
  (mirror `SCHEDULE_WAKEUP_DENY_REASON`'s "keep working" framing).

4.2 `build_command_builder` — append the tool list to `--disallowedTools` in
**every** permission mode, alongside the existing `ScheduleWakeup`.

4.3 `get_hooks` — add a `PreToolUse` hook matching `^Bash$` →
`DENY_BACKGROUND_BASH_CALLBACK_ID` in all three modes. Extend the catch-all
negative-lookahead matchers to exclude the newly denied names.
- The `Bash` matcher is *not* a blanket `Bash` deny: the callback decides.

4.4 `claude/client.rs::on_hook_callback` — handle the new callback id: deny only
when `tool_input.run_in_background == true`; otherwise fall through to normal
handling. Place the check **before** the `auto_approve` short-circuit, exactly as
`ScheduleWakeup` is, so it fires in bypass/yolo mode.
- `run_in_background` absent or `false` ⇒ allow. Parse defensively; a malformed
  `tool_input` must not deny ordinary `Bash`.

4.5 Tests, mirroring `schedule_wakeup_denied_in_all_hook_modes` /
`schedule_wakeup_disallowed_in_all_command_modes`:
- background-`Bash` denied in all three hook modes;
- foreground `Bash` still allowed (**the regression that matters** — an
  over-broad deny breaks every agent);
- poller tool names present in `--disallowedTools` in all command modes;
- catch-all matchers exclude the new names.

## Layer 5 — Close Codex's internal poller

`crates/executors/src/executors/codex.rs`

5.1 In the `ThreadStartParams` config map, set `features.unified_exec = false`.
5.2 Test pinning the exact key string — unknown keys are silently ignored
upstream, so a typo is an inert control. Assert against the vendored
`codex-app-server-protocol` types where possible rather than a bare string
literal.
5.3 Do **not** touch `features.shell_tool` (would break ordinary execution).

## Layer 6 — Grok: no code change

6.1 Add a comment in `crates/executors/src/executors/grok.rs` (or the ACP client)
recording that terminal capability is deliberately not advertised
(`ClientCapabilities.terminal = false`, all five `terminal/*` methods stubbed
`method_not_found`), so the absence reads as a decision rather than an oversight.
No deny rule — no verified tool name. Tracked as SPEC open question 1.

## Layer 7 — Right drawer

7.1 `packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx` — mount
`ExecutionProcessesProvider` for the selected session around the desktop **and**
mobile `RightSidebar` mounts, so the drawer reads the existing stream.
- Verify this does not double-mount for a session already provided upstream; if
  the chat already provides it at a common ancestor, hoist rather than add.

7.2 `packages/web-core/src/shared/stores/useUiPreferencesStore.ts` — add
`PERSIST_KEYS.pollersSection` **and** the matching `PersistKey` union member (the
union is hand-maintained; omitting it is a type error).

7.3 `PollersHeader.tsx` — count of `run_reason === 'backgroundhelper' && status
=== 'running'` from `useExecutionProcessesContext()`. Returns `null` at zero. No
query of its own (Constitution XXVI). Explicit truncation boundary; `title` +
`aria-label`, following `GitBehindHeader`.

7.4 `PollersSectionContainer.tsx` — list rows: command, interval, status,
started-at, stop action. Derived from the same context; the stop action calls the
existing `executionProcessesApi` stop method. Presentation via `@vibe/ui`
primitives — the container must not reimplement presentation (Constitution IV).

7.5 `RightSidebar.tsx` — new `SectionDef` with `fillAvailableSpace: true`,
`headerExtra: <PollersHeader/>`; add every new dependency to the `useMemo` dep
list. No `min-h-[Npx]`.

7.6 `shared/lib/api.ts` — poller methods via `handleApiResponse<GeneratedType>`,
following the `workerNodesApi` shape.

7.7 Tests — extend `RightSidebar.test.tsx`:
- count present when collapsed (mirror the existing Git-header test);
- absent at zero;
- section root classes follow the `fillAvailableSpace` contract.
Run via `pnpm test` (the `NODE_ENV=production` `act()` gotcha).

## Layer 8 — Verify, document, review

8.1 `pnpm run generate-types` (or `generate-types:check`).
8.2 `pnpm run check`, `pnpm run lint`, `cargo test --workspace`, `pnpm test`.
8.3 `pnpm run format` (required before completing a task).
8.4 **Behavioural verification, not just green tests.** Launch the app, start a
poller via the MCP tool, confirm: it appears in the drawer collapsed count and
expanded list; it survives turn end; a background `Bash` is denied with the
redirect reason while a foreground `Bash` still runs.
8.5 Independent Codex review of the diff; iterate until no significant findings.
8.6 Knowledge base: new page on the internal-poller replacement policy + vk
poller surface; add this task id to `agent-process-lifecycle.md` and
`flexible-collapsible-panel-stacks.md`; refresh `wiki/INDEX.md`.

## Risk order / rollback

Layers 1–3 are additive (new tools + an optional field) and safe alone. Layers
4–5 change agent behaviour and are the rollback-first candidates: reverting the
`--disallowedTools` additions and the `Bash` hook restores today's behaviour
exactly. Layer 7 is presentation-only.

The highest-risk single change is 4.4 — an over-broad `Bash` deny would break
every Claude execution. Its "foreground Bash still allowed" test is the gate.
