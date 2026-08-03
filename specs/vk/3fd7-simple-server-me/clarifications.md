# Clarifications: Cluster Server Metrics (`3fd7-simple-server-me`)

Five `[NEEDS CLARIFICATION]` markers were raised in `spec.md`. All five are
resolved below and folded into the spec. Each records the decision, the
reasoning, and what would reopen it.

---

## C1. Should command-line masking be relaxed to show full arguments?

**Decision: No. Masking stays, and it is not configurable.**

The premise of the question — "the operator already has shell access to these
hosts" — does not hold. Vibe Kanban's web UI is reachable through a public
Cloudflare tunnel, and app access is not the same principal as `root` on
`think2`. Anyone who can open the app would be able to read every command line
on every node in the cluster, including command lines belonging to processes
Vibe Kanban did not start.

Constitution XIX now states the rule directly: process environments are never
read, and command lines are redacted at the point of collection. A
configuration switch that turns redaction off is a switch that turns a
disclosure on, and would have to be secured, audited, and reasoned about
forever. The cost of the decision is asymmetric — an over-redacted command is
cosmetic, an under-redacted one is a leak — so there is no version of this
where shipping the unmasked variant is the careful choice.

The redactor's masking runs **inside the sampler on the node**, so an unmasked
command line never crosses the wire, never enters the coordinator's memory, and
never reaches a log line.

**Reopens if:** the metrics surface becomes gated behind a distinct
host-administration permission that is provably not equivalent to ordinary app
access.

---

## C2. Should the panel overlay the application content, or push it aside?

**Decision: Overlay.**

The panel must be reachable from anywhere (FR-8), which includes the workspaces
layout. That layout is already a `react-resizable-panels` group with a docked
right sidebar whose width is persisted (`usePaneSize`). A second reflowing
column would have to negotiate with that group on every page, would change the
content width of every route that does *not* have such a group, and would make
"open the metrics panel" a layout-mutating action on pages where the user is
mid-task — which FR-9 explicitly forbids.

An overlay is also the reversible choice: it is a portal sibling with no
relationship to any route's layout, so removing it removes nothing else. The
repository already has exactly this component to mirror (`MobileDrawer`), which
keeps the change small (Constitution III).

The cost is that the panel covers content while open. That is acceptable for a
transient diagnostic view that the operator opens, reads, and dismisses, and it
is mitigated by the panel being dismissible with `Escape` and a backdrop click.

**Reopens if:** operators report wanting the panel open continuously alongside
their work, in which case a docked mode becomes a follow-up, not a
reinterpretation of this one.

---

## C3. Should the refresh rate be operator-adjustable?

**Decision: Fixed at 2 seconds, defined as a single constant.**

`btop`'s adjustable rate is attractive, but the cadence in this design is owned
by the *sampler on each node*, not by the viewer. Making it adjustable from the
browser means propagating a rate through the coordinator to every worker's
sampler, which turns a UI preference into a protocol and configuration change —
and means one operator's choice silently changes what another operator sees,
since the samplers are shared.

Two seconds is `btop`'s own default, and it is the rate at which per-core CPU
deltas are meaningful without the `/proc` walk becoming a measurable cost.

The constant lives in one place so that changing it later is a one-line change
rather than an archaeology exercise.

**Reopens if:** operators need sub-second resolution to catch short spikes, in
which case the right design is a per-node "burst" mode requested for a bounded
duration, not a free-running adjustable global rate.

---

## C4. How much history should the rolling window hold?

**Decision: 150 samples ≈ 5 minutes. History holds everything *except* the
process table.**

Five minutes is the span that answers the question the graphs exist to answer —
"is this a spike or has it been like this?" — and it fills a sparkline at the
panel's default 420px width at a legible density.

The second half of this decision matters more than the first. A naive
implementation retains the whole `HostSample`, and the process table dominates
it: 15 processes at roughly 200 bytes each is ~3 KB per sample, versus well
under 1 KB for everything else combined. Retaining it for 150 samples costs
~450 KB per node and ~1.4 MB across a three-node cluster, all of it to store
data nobody graphs — no panel plots a process table over time.

So the ring retains the CPU, memory, filesystem, and network series, and the
process table is carried on the **latest sample only**. That drops per-node
retention to roughly 150 KB and keeps the streamed patch payload small, which
is what Constitution XIX's bounded-stream rule is protecting.

**Reopens if:** a use case appears for per-process history (e.g. "which agent
was responsible for that spike three minutes ago"), which would want a
separate, deliberately-designed narrow series rather than retaining the whole
table.

---

## C5. When a node is unreachable, keep its last readings or blank the panel?

**Decision: Keep them, visibly stale, timestamped, with the reason — and expire
them once they age past the retention window.**

Blanking discards information the operator has already paid for. "CPU was at
94% as of 30 seconds ago, and the node has since become unreachable" is a
materially better diagnosis than an empty panel, and it is often the most
important reading in the session — a node that stops responding *right after*
its load spiked is telling a story.

This does not conflict with FR-17's ban on fabricated zeros, because a retained
reading is a real measurement presented as a past one. The distinction is
carried in the UI: the panel is visibly de-emphasised, the reason
(`unreachable`) is stated, and the readings are labelled with the timestamp
they were taken at rather than presented as current.

The expiry bound is what stops this from becoming a lie. Once the newest
retained sample for a node is older than the retention window (5 minutes), the
readings are dropped and the panel shows only the status and the time contact
was lost. Data more than five minutes stale is not evidence of anything, and
displaying it, however greyed, invites misreading.

**Reopens if:** operators find the stale readings confusing in practice, in
which case the mitigation is a stronger visual treatment, not deletion.

---

## Remaining open questions

None. All five markers are resolved.

One item was raised in `SPEC.md` and deliberately **not** carried into this
feature's scope: the two inconsistent React Query keys for worker nodes
(`['worker-nodes']` in the settings section, `['workerNodes']` in the carousel
and create-chat containers). Consolidating them is a real cleanup, but it is
unrelated to this feature and would put the workspace-placement UI in this
task's blast radius for no benefit. It stays out of scope and is noted in the
knowledge base instead.
