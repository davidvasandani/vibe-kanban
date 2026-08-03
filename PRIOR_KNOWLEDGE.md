# Prior Knowledge: Cluster Server Metrics (`3fd7-simple-server-me`)

The project knowledge base is populated. This repository carries **two**
knowledge bases and both were searched:

- `docs/knowledge-base/` (21 pages, `INDEX.md`) — the current one; task ids
  match `specs/vk/*`.
- `wiki/` (19 pages, `INDEX.md`) — an earlier generation, still authoritative
  for several frontend and lifecycle topics.

## Most relevant pages

| Page | Why |
| --- | --- |
| `docs/knowledge-base/clustered-workspace-execution.md` | The coordinator↔worker signed channel, authority split, and lease/health semantics this feature rides on |
| `wiki/browser-session-control-arbiter.md` | The closest precedent for a live WS-streamed subsystem; carries the serde float gotcha and the background-sweeper rules |
| `docs/knowledge-base/claude-log-normalization.md` | The repo's existing JSON-Patch-over-WebSocket dialect |
| `wiki/agent-process-lifecycle.md` | Long-lived background poll loops and their leak/termination traps |
| `docs/knowledge-base/collapsing-repeated-log-entries.md` | Memory bounds on high-frequency streams |
| `docs/knowledge-base/mcp-connectivity-testing.md` | Secret-safe bounded diagnostics, "no new dependency" precedent, `Unsupported` status discipline |
| `wiki/electric-sync-fallback.md` | Snapshot-vs-stream fallback and degradation UX |
| `wiki/self-hosted-deployment.md` | Release/binary layout and level-triggered-over-edge-triggered |
| `wiki/appbar-rail-and-org-tiles.md` | Persisted zustand UI state and tile/rail conventions |
| `wiki/workspace-carousel-view.md` | Rendering N live WS-fed panes at once |
| `docs/knowledge-base/pipeline-settings-editor.md` | Host-scoped frontend state and stale-response guards |
| `docs/knowledge-base/workspace-directory-reclamation.md` | Rust filesystem-parsing traps that apply verbatim to a `/proc` walk |
| `wiki/mobile-kanban-scrolling.md` | The overflow rule for the drawer's scroll container |
| `wiki/kanban-issue-panel-sections.md` | Sectioned-panel conventions and the component-test recipe |
| `docs/knowledge-base/cli-tool-oauth-login.md` | Signed, machine-scoped WebSocket from the browser |
| `docs/knowledge-base/worktree-formatting-prerequisites.md` | Fresh-worktree setup and CI path filters |
| `wiki/workspace-context-bar-responsive-visibility.md` | Desktop-only floating chrome gating |
| `docs/knowledge-base/workspace-environment-inheritance.md` | Never log or debug-format resolved secrets |

## Hard constraints extracted for this task

### Protocol and authentication

1. **Sign the full request target including the query string.** "Worker requests
   are signed over timestamp, HTTP method, the full path and query, and a digest
   of the exact body bytes… Omitting the query permits replay against a
   different event cursor." `GET /v1/metrics?after=N` must sign the `?after=N`.
2. **Verify against `OriginalUri`.** Axum nesting rewrites the URI seen by inner
   middleware; a request signed as `/api/…` must not be checked as `/…`.
3. **Fresh timestamp and nonce per poll.** A polling collector that caches its
   signed envelope will be rejected as a replay. Never reuse an envelope.
4. **Explicit body/response cap before buffering.** A metrics payload with
   per-core and per-process arrays is not small; cap it, and account for
   encoding expansion.
5. **The browser never supplies a path, pid, or filter expression.** Everything
   the sampler reads is compiled in.

### Health, authority, and degradation

6. **Metrics are observations, never evidence.** "An offline or unreachable
   worker means the workspace is indeterminate, not idle." An unreachable node
   must render as a typed `unreachable` status, never as `0%`. No metrics path
   may write worker status, lease, or eligibility.
7. **Expire leases before listing nodes.** "Expiring stale `online` rows only
   inside scheduler selection leaves an admin UI claiming a dead worker is
   healthy." The metrics node list must call `expire_heartbeats` the way
   `list_workers` already does.
8. **A distinct `Unsupported { reason }` status, never a false failure.** Non-
   Linux hosts and unreadable `/proc` are their own status.
9. **Falling back is recovery, not an error.** A transparent WS reconnect and
   resnapshot must not raise a user-facing banner. And errors do **not**
   auto-clear — recovery needs an explicit clear signal that also resets the
   error-report debounce, or a fresh identical failure is debounced away.
10. **Level-triggered beats edge-triggered.** A periodic full snapshot is the
    convergence backstop; a pure patch stream stalls silently. Broadcast lag can
    drop messages, so a replay gap must force a resnapshot, not interpolation.

### Wire format

11. **⚠️ No `f64`/`f32` field inside a `#[serde(tag = "…")]` internally-tagged
    enum.** `serde_json` is built with `preserve_order` workspace-wide
    (`Cargo.toml:44`), and deserializing an internally-tagged enum whose variant
    has a float field fails with `invalid type: map, expected f64`. Integers and
    strings are fine. Keep all percentages/loads in plain structs, or scale them
    to integers.
12. **Reuse `json_patch::Patch`**, the existing dialect — don't invent a second.
13. **Key rows by stable identity, never array index.** "A stale stored index
    causes a `replace` that overwrites whatever entry got reallocated at that
    index after the reset — this exact bug was caught by Codex review." Nodes
    key by `node_id`; process rows key by `(pid, start_time)`, not position.
14. **Register every new ts-rs type in
    `crates/server/src/bin/generate_types.rs`**, regenerate with
    `pnpm run generate-types`, never hand-edit `shared/types.ts`. If any enum
    gets an `ALL` completeness list, add the variant there — omitting it makes
    the concept invisible while everything still compiles.

### Background sampling

15. **Bounded memory is non-negotiable.** "Never render an unbounded tick
    string… repeatedly building progressively larger replacement patches can
    exhaust the server's memory." Fixed-size ring, bounded top-N, and a patch
    payload whose size does not grow with uptime.
16. **The sampler task holds only a `Weak`** to its owner between ticks; a
    strong clone in the loop leaks the service forever.
17. **Check for a dead consumer every tick.** "Dropping the receiver does not
    stop the task — it must check `tx.is_closed()` each iteration or it spins
    forever."
18. **Never hold a lock across an await**; evict stale entries
    generation-conditionally after dropping the lock, or you can reap something
    that was re-registered in the window.
19. **Do downsampling and retention server-side**, so every client benefits and
    no client re-derives history.
20. **Do not persist samples to SQLite.** It is single-writer and this is
    per-second data.

### `/proc` parsing traps

21. **`read_dir(..).filter_map(|e| e.ok())` silently drops unreadable entries.**
    `/proc/[pid]` entries vanish mid-walk and are often unreadable for other
    users; dropping them turns "couldn't read" into "not there".
22. **`Path::exists()` returns `false` for both "absent" and "stat failed"** —
    use `try_exists()`.
23. **An errored sample is not a zero sample.** Model indeterminate explicitly.

### Secrets

24. **Redact longest-first and cap length** for anything sourced from
    `/proc/[pid]/cmdline`. Do not open `/proc/[pid]/environ` at all.
25. **Never log or debug-format the result** — not in error strings either.
    Stream samples; do not persist or log command lines.

### Dependencies and deployment

26. **Prefer no new dependency.** The precedent (`mcp-connectivity-testing`) is
    a hand-rolled probe built on crates already present, after explicitly
    checking whether an existing crate could do it. Pulling in `sysinfo` /
    `procfs` needs a written justification or expect a review question.
27. **Do not add a sixth binary.** `local-build.sh` publishes a fixed
    `build-<id>/bin/*` set; a new binary that isn't published simply is not
    deployed. Put the collector in a library crate consumed by the existing
    `server` and `vibe-kanban-worker` binaries.
28. **No writes at service start**, and no state under the source checkout.
29. **Add the new crate to the CI path filters.** "Adding a test command to a
    filtered job is insufficient if changes to the tested files do not trigger
    that job."

### Frontend

30. **Host-scope the query keys.** Per-machine data must go through the selected
    machine client (`makeHostAwareRequest` / `queryScopeKey`), or one host's
    data renders under another. Guard with a `useRef` so a late response for a
    deselected host is dropped.
31. **Dedicated persisted zustand store** with its own key (à la
    `useOrgRailStore`). `useExpandableStore` is deliberately not persisted —
    don't overload it.
32. **Bound the websocket count** — one multiplexed stream for all nodes, not
    one per node or per sparkline.
33. **Per-node error boundaries.** With N panes rendered at once, "one workspace
    with bad data must not blank the entire view."
34. **A vertical scroller must also carry `overflow-x-hidden`** — `visible` on
    one axis combined with a scrolling value on the other computes to `auto`,
    silently creating a horizontal scroller.
35. **Debounce starvation trap:** an effect that re-arms its debounce timer in
    its cleanup resets the countdown on every unrelated update. Compare content
    before touching the timer — high-frequency metric patches will hit this.
36. **Gate the drawer's hooks at a wrapper** so mobile never mounts the
    subscription, rather than conditionally calling hooks.
37. **Derive presentation colour client-side**; don't add a data field just for
    tile/series colouring.
38. **Don't render a `<button>` with a no-op `onClick`** — use a
    non-interactive element with an `aria-label`.
39. **The WebSocket constructor can succeed before its HTTP upgrade is
    rejected**, so handle `error` and premature `close`, or the drawer sits in
    "connecting" forever.
40. **Component tests:** run via the package script so `NODE_ENV=test` is set —
    the dev environment exports `NODE_ENV=production`, which makes
    testing-library fail with "`act(...)` is not supported in production
    builds". `@vibe/ui` component tests run from the `remote-web` package.
    Without an i18n provider `t()` returns raw keys. Use
    `vi.advanceTimersByTimeAsync(ms)`, not `runAllTimers`, for interval-driven
    code.

### Verification

41. `pnpm install --frozen-lockfile` first in a fresh worktree; then
    `cargo test --workspace`, `pnpm run check`, `pnpm run lint`,
    `pnpm run generate-types:check`, `pnpm run format`.
42. **A two-node deployment exercise is the real gate.** "Passing local tests
    does not replace that deployment gate."

## Implications for this task

- The `NodeMetricsAvailability` enum in `SPEC.md` §1 is internally tagged and
  must stay float-free (constraint 11). All percentages live in plain structs.
- `SPEC.md` §4's collector must additionally: call `expire_heartbeats` on the
  node list (7), hold a `Weak` and stop on zero subscribers (16, 17), sign a
  fresh envelope per poll (3), cap the response body (4), and emit a periodic
  full resnapshot as the convergence backstop (10).
- `SPEC.md` §2's process walk must not `filter_map(ok)` over `/proc` (21) and
  must key processes by `(pid, start_time)` for patch stability (13).
- `SPEC.md` §7's frontend must host-scope its endpoint (30), carry
  `overflow-x-hidden` (34), gate hooks behind a mobile wrapper (36), wrap each
  node panel in an error boundary (33), and handle WS `error`/`close` (39).
- The new `crates/node-metrics` must be added to the CI path filters (29) and
  must not become a new binary (27).
