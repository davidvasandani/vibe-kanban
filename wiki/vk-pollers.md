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

Claude Code 2.1.268's background poll loop is
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

## A block is not a redirect — deliver both, per agent

Shipped first time for Claude and **missed for Codex**, which is why "Codex isn't
using the poller" was reported after the feature landed.

Claude's control is self-announcing: the `PreToolUse` deny fires at the exact
moment the agent reaches for the wrong tool, and its reason names `spawn_poller`.
Block and redirect arrive together.

Codex's control is **silent**. `features.unified_exec=false` just swaps the
persistent-PTY `exec_command`/`write_stdin` pair for one-shot `shell_command` —
no error, no message, nothing for the agent to notice. Codex sees a command that
returns immediately, has no idea a poller exists, and carries on. The block
landed; the redirect never did.

The delivery channel is `ThreadStartParams.developer_instructions`, which the
protocol offers alongside `base_instructions`. Two constraints:

- **Compose, never clobber.** `developer_instructions` is a user-facing config
  field; VK is a guest in it. The notice is prepended and the operator's text
  goes last, so it is the most recent thing Codex reads and can override.
- **Don't use `base_instructions`** to carry this — it replaces Codex's whole
  system prompt.

The general rule: when a capability is removed, check *how the agent finds out*.
If the mechanism is a config flag rather than a refusal, nothing tells it, and a
correct block still produces the behaviour you were trying to fix. Constitution IX
requires the replacement to be named — that obligation is per-agent, and it is
easy to satisfy for the agent with a deny hook and forget for the one without.

## Verify vendor identifiers against the artifact that executes

This task nearly shipped two inert controls. Constitution IX was extended
(0.30.0) to require the discipline.

- **Claude:** `@anthropic-ai/claude-code@2.1.268` installs the platform package
  such as `@anthropic-ai/claude-code-linux-x64`; that package's native binary is
  what executes. The wrapper ships `sdk-tools.d.ts`, which looks authoritative
  but lists **JSON-Schema titles, not wire tool names** (`FileReadInput` → real
  tool `Read`). A deny-list built from it would match nothing and fail silently.
  Names must be read from the binary. It also contains an alias→canonical map
  (`BashOutput`/`KillShell` → `TaskOutput`/`TaskStop`), and the permission parser
  normalizes through it, so denying canonical names covers aliases.
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

## Assert what the caller receives, not what the code returns

The poller routes shipped with tests that asserted the rejection *variant* —
`InvalidInterval`, `InvalidWorkingDir` — and those tests passed the whole time.
Driving the shipped `spawn_poller` MCP tool against the running service showed
every rejection arriving at the agent as:

```json
{"error": "VK API returned error", "details": "Unknown error"}
```

`ApiResponse::error_with_data` sets `message: None` by construction. The frontend
matches on the typed `error_data`, but **non-browser clients only surface
`message`** — so any route an MCP tool can reach must use
`error_with_data_and_message`, or it is unusable at its own error boundary.

Two transferable lessons:

- A test that asserts an internal enum is not a test of the contract. The
  contract is what crosses the boundary. Here the variant assertion and the user
  experience disagreed completely, and only the variant was covered.
- For an agent-facing denial this is not cosmetic. The whole feature exists to
  redirect agents away from their CLI's in-turn mechanism; an agent told
  "Unknown error" cannot tell whether to change the interval, the working
  directory, or stop retrying, so the redirect fails exactly where it matters.
  Constitution XXI ("failures say what failed") and the Principle IX requirement
  that a denial name its replacement are the same rule applied at two layers.

Note `spawn_background_helper` still has the message-less shape through the same
constructor — pre-existing, and left for its own change rather than folded in.

## Post-result hook grace is a quiescence window

Claude's structured-input stdin is both the prompt stream and the return path
for PreToolUse callbacks. VK must eventually close it so the natural-exit CLI
can terminate, but a `result` line is not proof that all SDK-managed/background
activity and hook traffic has drained.

The original late-Stop-hook fix kept stdin open until an absolute deadline 500
ms after the first result. VAS-540 exposed the remaining race: Claude could emit
another structured line inside that window and then request
`DENY_BACKGROUND_BASH_CALLBACK_ID` less than 500 ms later, while the original
timer still expired between them. The CLI then reported `Stream closed` before
VK ever received the callback.

Treat `POST_RESULT_GRACE` as **time since the latest non-empty output**, not
time since the result. Once a result arms shutdown, every later line resets the
deadline. Control requests remain handled inline and their response is flushed
before the timer is checked again. This preserves bounded natural exit while
making visible activity authoritative over an old timer.

Two reusable testing rules:

- Value tests for a hook's deny JSON cannot prove transport delivery. Use a
  duplex protocol test that crosses the old deadline and reads the matching
  `control_response`.
- A mandatory response-write failure is a protocol-loop failure, not a warning
  that can be swallowed while the execution appears successful.

## Not this: a wake-up scheduler

A vk poller runs a **command**, not a turn; it never resumes the agent. Persisting
`(session_id, fire_at, prompt)` and re-invoking via `--resume` +
`QueuedMessageService` is VAS-283 "option B", deliberately deferred alongside
VAS-132. Don't reopen it incidentally — see [[agent-process-lifecycle]].

## Workspace sidebar grouping uses bulk workspace state

The Polling group takes its signal from
`WorkspaceSummary.has_running_poller`, mapped to `hasRunningPoller` by
`useWorkspaces`. The summary query joins executions through **all sessions**
of each workspace; inspecting only the latest session loses older live loops.
It reuses the existing 15-second summary refresh rather than adding one
execution subscription per sidebar row.

A poller is a running `backgroundhelper` whose root action is
`ScriptRequest` with object-valued `typ.poller` metadata. A sleeping interval
still belongs to a running loop. A plain helper, dev server, or terminal
execution does not count. Guard SQLite JSON extraction with `json_valid` so a
malformed legacy row cannot fail the entire summary.

**Dropped history is not stopped execution.** The existing `list_pollers`
API includes live processes even when their chat history is hidden. The
bulk query follows that same rule; do not add `dropped = FALSE` merely
because latest-agent-turn queries use it.

For polling workspaces, sidebar precedence is pending approval → active agent
run/provisioning → Polling. Thus unread completion output alone does not
require attention while monitoring continues. Preserve the old predicate for
non-polling workspaces, and never change read receipts to implement grouping.
Carousel ordering remains independent.

The Polling section owns a separate collapse key and uses the shared header
and workspace list. Regression tests belong in the existing remote-web DOM
harness, which renders the same shared sidebar as local-web; backend tests
exercise the bulk query against migrated SQLite and serialized executor actions.

## Automatic stopping rules

**Task:** `vk/bd71-require-all-poll`  
**Workspace:** `bd7136e0-1243-44dd-846e-443381de0aca`

New pollers require `stop_command`, positive `timeout_secs`, or both. A stop
command is a completion predicate checked **before** each tick: zero stops;
nonzero continues. Blank supplied predicates and zero supplied limits are
rejected even when the other field is valid. Manual `stop_poller` availability
does not satisfy creation validation. Use both rules when a predicate or tick
might hang.

The new fields remain optional in persisted `PollerSpec` JSON so old rows are
readable. Existing live legacy processes keep their original script; compiling
legacy metadata without either rule applies a one-day fallback. HTTP/MCP list
responses and the existing execution stream carry the fields; the drawer still
adds no request.

### A deadline belongs to the process group

A server-owned timer would disappear on restart or reset during recovery. The
compiled script instead starts a Bash/sleep watchdog in the same process group
that `ScriptRequest::spawn` owns. `$$` identifies that group's leader. Re-adopting
the live group preserves the original deadline. Predicate completion cancels
and reaps the watchdog, whose cancellation trap kills and reaps its sleep.
Deadline expiry logs its reason and kills the whole group, including resistant
ordinary descendants. Deliberately escaping the group is outside this contract.

Two tested pitfalls explain the implementation:

- GNU `timeout` is not a standard macOS runtime dependency. Reuse the Bash and
  sleep tools already required by pollers; lifecycle fixtures deliberately omit
  timeout/gtimeout from PATH.
- TERM plus delayed KILL can lose grandchildren: the supervisor may exit when
  the monitored shell exits on TERM, before escalating against a descendant
  that ignored TERM. Test with a **separate shell** that ignores TERM and writes
  a marker after the deadline; a superficially similar inline background/wait
  fixture failed to expose this leak.

### Cleanup must survive the stopping mechanism

SIGKILL skips EXIT traps. Scratch output/status therefore live in one atomically
created private directory. The watchdog removes that directory **before** group
termination. Removing only the files has a race: a finishing tick's EXIT trap
can recreate its status file between unlink and KILL. Removing their parent
directory prevents recreation, and open unlinked output files disappear when
the killed processes close their descriptors.

Regression tests assert empty scratch storage after both timeout and successful
predicates, no surviving watchdog on successful completion, and no delayed
writes from resistant descendants. The fixtures use Bash's kill builtin for
process-group cleanup rather than assuming the host's external kill utility has
the same negative-PID behavior.

## Bounding an effect, not a name

**Task:** `vk/603d-prevent-stuck-jo`

The two controls above — the `Monitor`-family tool-name denial and the
`run_in_background` parameter denial — both worked, and a turn still hung for
about an hour. The agent asked for `Monitor`, was denied, and fell through to a
plain **foreground** loop:

```
until grep -q "### restored:" /tmp/out.log; do sleep 5; done
```

That matches no tool name and no parameter. The earlier lesson was "the
chokepoint is a parameter, not a tool name"; this is the next step in the same
sequence, and the general form is:

> A control removes an **effect**, not a name. Enumerate the reachable paths to
> the effect and bound each. Removing the ergonomic path while an equivalent one
> stays open displaces the incident — and the recurrence looks unrelated to the
> control you shipped, so nobody connects them.

Constitution IX was extended (0.32.0) to require this.

### Two layers, and only one of them is load-bearing

- **Refuse the wait at admission** (load-bearing). Same `^Bash$` matcher, same
  `DENY_BACKGROUND_BASH_CALLBACK_ID`, second predicate. No new callback id,
  matcher or hook registration, so it inherits bypass/yolo coverage for free —
  and bypass is the mode incidents happen in.
- **A VK-owned command bound** (backstop) for shapes the predicate misses.

### The timeout env vars, and the clamp that makes a tightening inert

Verified in the pinned `@anthropic-ai/claude-code@2.1.268` native binary:

```js
zRo = 120000, qRo = 600000;
pAe(env) // effective default: env.BASH_DEFAULT_TIMEOUT_MS or zRo
qYe(env) // effective max: max(env.BASH_MAX_TIMEOUT_MS, pAe(env)) else max(qRo, pAe(env))
```

Three things that are silent if got wrong:

- **The maximum is clamped to at least the effective default.** Lowering
  `BASH_MAX_TIMEOUT_MS` *alone* below the default does **nothing**. VK pins
  `max >= default` with a `const` assertion, not a test, so an inert control
  breaks the build.
- A non-numeric or non-positive value is **ignored**, falling back to the
  built-in default with no error.
- At the deadline the CLI may **detach** the command rather than kill it
  (`canAutoBackground`). So the bound reliably unblocks the *turn*; it is not a
  kill. Anything left running is reaped by the turn-end group kill.

VK sets both variables to the vendor's *current* values on purpose. The win is
ownership and bump-resistance — `@anthropic-ai/claude-code` is a `needs-review`
Renovate carve-out — not tightening. Tightening was rejected on evidence:
`cargo test --workspace` and `pnpm install` legitimately exceed five minutes, so
a shorter cap breaks real work while the admission refusal already catches the
loop far earlier.

They are **defaults on both sides**, which takes two separate mechanisms and
only one of them is obvious. Seeding happens *before* `apply_to_command`, so a
profile or organisation variable of the same name is applied after and wins —
but `ExecutionEnv` is built from org variables and VK context, **not** from
`std::env`, so that alone does not cover a variable an operator exported for the
VK service itself. A child `Command` inherits the server's environment, and an
unconditional `.env()` would quietly override it. So seeding also skips any
variable already set. Getting only the first half right leaves a documented
escape hatch that does not exist for the case an operator is most likely to use.

### A deny predicate on the workhorse tool is mostly false-positive engineering

`Bash` runs everything, so an over-broad deny is worse than the bug. The rule is
three independent conditions — loop keyword **and** `sleep` **and** no bounding
marker — plus `watch` as the leading token only. `for` loops are never denied.

**Every one of these was wrong on the first attempt.** The predicate looks like
five lines of string matching; it is actually where all the risk lives, and the
review pass found four separate defects in it after the tests were green. Budget
for that.

- **Token matching is not enough for `until`/`while`: they are English words.**
  `echo "retrying until ready"; sleep 2` has a keyword, a sleep and no marker,
  and was refused. The keyword must be in **command position**.
- **`trim_end()` silently deleted the newline separator.** Command position
  allows "after a newline", but trimming the preceding text with `trim_end()`
  strips the newline first, so that arm was unreachable. Any multi-line
  script — the ordinary way to write a wait loop — escaped the guard entirely.
  The load-bearing layer was off for the shape it exists to catch, and every
  test still passed because they were all single-line. Trim blanks only.
- **A comparison is not a counter.** `-lt`/`-gt` were markers, on the theory
  that they indicate `while [ $n -lt 5 ]`. But they read identically in a
  *polling condition*: `until [ $(grep -c ready "$f") -gt 0 ]; do sleep 5; done`
  is semantically the incident command and was allowed. The distinguishing
  feature is an **increment**, so the marker is `$((` — which also does not
  collide with the plain `$(` a polling condition uses.
- **Markers must be matched where they mean something.** Bare-token `timeout`
  matched the polled path in `until test -f /tmp/timeout.flag; do sleep 5; done`
  and marked it bounded. It counts only in command position.
- **`while read` with a `sleep` is bounded.** A rate-limited
  `cat urls.txt | while read -r u; do curl "$u"; sleep 1; done` terminates when
  its input does; it was refused. `read` is a marker.

**Treat `-` as a word character** throughout: it makes `--timeout=5` a single
token that does not match the `timeout` command (bounding one attempt does not
bound the loop) and keeps `git log --until=…` from reading as a loop.

Accepted, documented limits: a heredoc that *writes* a wait loop is refused as
if running it; `eval`/variable-assembled loops do not match; a busy loop with no
`sleep` is bounded only by the command timeout; and a marker anywhere shadows
the whole command, so markers signal *intent* to bound rather than proof of one.
Ambiguity resolves permissively by design.

### Scope is per-agent, and an absence must read as a decision

Claude only, and the Codex half is now **verified**, not merely deferred.

Codex has no `PreToolUse` equivalent to attach a refusal to, so layer 2 has no
attachment point. For layer 1, the vendored `rust-v0.144.1` source
(`44918ea…`, the tag Cargo already pins for `codex-app-server-protocol` — the
authoritative artifact, not the npm tarball) shows:

- `core/src/exec.rs:58` — `DEFAULT_EXEC_COMMAND_TIMEOUT_MS: u64 = 10_000`
- `core/src/tools/handlers/shell_spec.rs:171` — the `shell` tool's `timeout_ms`
  parameter, "Maximum command runtime. Defaults to 10000 ms."
- `core/src/exec.rs:163` — `From<Option<u64>> for ExecExpiration`: absent →
  `DefaultTimeout`, supplied → `Timeout(..)`

The default is a **hard-coded `const`**. `codex-rs/config.md` documents no
execution timeout, and the only timeout keys under `codex-rs/config/src` are MCP
ones (`startup_timeout_sec`, `tool_timeout_sec`). So there is no identifier for
VK to set and, per Constitution IX, **no rule ships** — with the evidence
recorded in code next to `features.unified_exec`.

Two things worth carrying forward:

- **Verifying an absence can also lower your estimate of the risk.** Codex turns
  out to be far less exposed than Claude was: every exec carries an expiration,
  the default is ten seconds rather than two minutes, and there is no
  auto-background path that outlives the deadline. The deferral had been written
  as though Codex were equally exposed and merely unchecked.
- **Record the residual.** A model-supplied `timeout_ms` is *not* clamped, so
  Codex can ask for one arbitrarily long command. That is one explicitly bounded
  call rather than an open-ended block, and closing it would need an upstream
  change — so it is documented, not patched around.

Grok's existing verified absence is unchanged.

### Drive the shipped control from inside a real turn

The cheapest high-value check on an agent-facing control, and the last one this
task did: once it is deployed, **use it**. A VK agent turn is itself a live
Claude process behind the deployed hooks, so a handful of ordinary `Bash` calls
exercise the whole path — server-side predicate, hook transport, and the text
the agent actually receives — in a way no unit or protocol test reaches.

Two design rules make it safe and worth doing:

- **Write deny probes whose condition is already true.** `until [ -f /etc/hostname ];
  do sleep 1; done` is refused if the guard is live and *exits immediately* if it
  is not. A probe that could hang is a probe you cannot run against the failure
  you are testing for.
- **Probe both directions.** Denying the bad shape proves the control exists;
  allowing the retry loop, the `while read … sleep` loop and the prose mention
  proves it is not over-broad. On the workhorse tool the second half is the one
  that would hurt.

This found nothing new — but it is what upgraded "the tests pass" to "the
deployed thing refuses the incident command and allows ordinary work", including
a live before/after on the two escapes fixed after the first merge. It also
confirmed layer 1 by reading `BASH_DEFAULT_TIMEOUT_MS` / `BASH_MAX_TIMEOUT_MS`
straight out of the agent's own environment.

What it still is not: independent review. It is the implementer driving their own
artifact, which raises confidence that the control *works*, not that it is right.

## Contributed by

- vk/869c-vk-background-po
- vk/5cd1-debug-this-vk-ba

- vk/dc76-add-polling-to-w

- vk/bd71-require-all-poll

- vk/603d-prevent-stuck-jo
