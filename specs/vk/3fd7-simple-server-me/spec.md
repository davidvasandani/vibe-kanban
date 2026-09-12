# Feature Specification: Cluster Server Metrics

**Feature dir**: `specs/vk/3fd7-simple-server-me/`
**Status**: Draft

## Summary

Operators of the self-hosted Vibe Kanban cluster have no in-app view of how
their machines are doing. Today the product shows two numbers per worker (load
average and active execution count) buried in Settings, and nothing at all about
the coordinator's own host. Answering "why is this node slow", "which agent is
eating the CPU", or "are we about to run out of disk on the shared mount" means
opening a terminal and SSHing into each box to run `btop`.

This feature brings that view into the product: live CPU, memory, disk, network,
and top-process readings for **every** node in the cluster — the coordinator and
each worker — in a drawer that slides in from the right edge of the app and can
be opened from anywhere. It is a read-only window. It changes nothing about how
work is scheduled, where workspaces land, or whether a node is considered
healthy.

## User Stories

- As a cluster operator, I want to see live CPU, memory, disk, and network usage
  for every node in one place, so that I can tell at a glance which machine is
  saturated without SSHing into three hosts.
- As a cluster operator, I want to see the top processes on a node ranked by CPU,
  so that I can tell whether a coding agent, a build, or something unrelated is
  responsible for the load.
- As a cluster operator, I want per-core CPU detail and a short history graph,
  so that I can distinguish a single pegged core from genuine whole-machine
  saturation, and a momentary spike from sustained load.
- As a cluster operator, I want to see disk usage per filesystem including the
  shared NFS mount, so that I find out about a filling volume before a workspace
  fails to write.
- As a cluster operator, I want a node that is unreachable to say so plainly,
  so that I never mistake "we could not read this machine" for "this machine is
  idle".
- As a cluster operator, I want the panel to open next to whatever I am already
  doing and remember how I left it, so that checking cluster health does not
  cost me my place in the app.
- As a security-conscious operator, I want the process list to never expose
  credentials that happen to sit in a command line, so that opening a monitoring
  panel is not a way to leak a token.

## Functional Requirements

**Coverage**

- FR-1: The system MUST report metrics for every node in the cluster: the
  coordinator host and each registered worker host.
- FR-2: The coordinator MUST be represented even when clustering is disabled, so
  a single-machine deployment still sees its own host.
- FR-3: Representing the coordinator MUST NOT create or alter a worker record.

**What is measured**

- FR-4: For each node the system MUST report, where the host can supply it:
  overall and per-core CPU utilisation; load averages over 1, 5, and 15 minutes;
  total, used, available, and cached memory; swap usage; per-filesystem total,
  used, and available space; per-interface network throughput and lifetime
  totals; and the top processes ranked by CPU with their name, owner, memory,
  thread count, and command.
- FR-5: Where a host cannot supply a reading (a missing sensor, an unreadable
  file), that reading MUST be reported as absent, distinctly from a reading of
  zero.
- FR-6: The system MUST retain a short rolling history per node — enough for a
  history graph of the last few minutes — and MUST NOT retain more than that.
- FR-6a: The rolling window holds approximately five minutes of readings. The
  process table is **not** retained in history — only the most recent process
  table is kept, because no view plots processes over time and retaining the
  table dominates the memory cost of the window. (C4)
- FR-7: Readings derived from a rate of change (CPU utilisation, network
  throughput) MUST be computed against a real prior observation. Before one
  exists, they MUST be reported as not-yet-available rather than as zero or as a
  spike.

**Presentation**

- FR-8: The metrics view MUST be reachable from anywhere in the application, not
  only from a particular page or project.
- FR-9: The metrics view MUST open as a panel anchored to the right edge of the
  window and MUST NOT navigate away from, or discard the state of, whatever the
  operator was doing.
- FR-9a: The panel **overlays** application content; it does not push content
  aside or reflow the page. Opening it MUST NOT change the layout of the page
  beneath it. (C2)
- FR-10: The view MUST list all nodes together with a compact per-node summary,
  and MUST allow selecting one node to see its full detail.
- FR-11: Each node MUST be identified by hostname and by its role (coordinator
  or worker).
- FR-12: Numeric readings MUST be accompanied by a proportional visual
  indicator, and time-varying readings by a short history graph.
- FR-13: The operator's view preferences — whether the panel is open, its width,
  which node is selected, and which sections are expanded — MUST survive a page
  reload.
- FR-14: The view MUST be usable by keyboard and screen reader: it MUST be
  dismissible by keyboard, MUST return focus where it came from, and every
  graphical indicator MUST carry a text equivalent of its value.

**Freshness and failure**

- FR-15: Readings MUST update continuously while the view is open, on the order
  of once every couple of seconds.
- FR-15a: The refresh rate is **fixed at two seconds** and is not
  operator-adjustable. The cadence belongs to the sampler on each node and is
  shared by every viewer, so it is not a per-viewer preference. (C3)
- FR-16: When the view is closed, the system MUST NOT continue collecting from
  remote nodes.
- FR-17: A node whose metrics cannot be obtained MUST be shown with an explicit
  reason — unreachable, unsupported platform, or not supported by that node's
  version — never as zeroed readings.
- FR-18: A node's last known readings MUST continue to be shown while it is
  unreachable, visibly de-emphasised, labelled with the reason, and timestamped
  with when they were taken rather than presented as current. Once the newest
  retained reading for that node is older than the retention window, the
  readings MUST be dropped and only the status and the time contact was lost
  shown — data that stale is not evidence and inviting it to be read as current
  is worse than showing nothing. (C5)
- FR-19: Failure to obtain metrics from one node MUST NOT affect the readings
  shown for any other node, and MUST NOT blank the view.
- FR-20: A transient interruption that recovers on its own MUST NOT be surfaced
  to the operator as an error. A failure to establish any connection at all MUST
  be surfaced, and MUST clear once data arrives.
- FR-20a: Once readings have been received, a subsequent interruption is
  reported as **staleness on the affected nodes** (FR-18), not as a view-level
  error. This matches what the shared streaming hook provides; distinguishing
  "recovered" from "degraded but holding stale data" at the view level would
  require changing a hook shared by five other features, which is out of scope
  here. (analysis W4)

**Boundaries with existing behaviour**

- FR-21: Nothing in this feature may change whether a node is considered online,
  draining, healthy, leased, or eligible to receive work.
- FR-22: Nothing in this feature may change where a workspace is placed, or
  affect a workspace or execution already running.
- FR-23: The view MUST NOT offer any action that changes the state of a node or
  a process. It is observation only.
- FR-24: Node health as displayed here MUST be consistent with node health as
  displayed elsewhere in the product; a node whose lease has expired MUST NOT
  appear healthy in one place and dead in another.

**Confidentiality**

- FR-25: The system MUST NOT read process environment variables.
- FR-26: Credential-like content in a process command line — tokens, API keys,
  passwords, secrets in URLs — MUST be masked before that command line leaves
  the machine it was read on, so that no unmasked value is transmitted, stored,
  or logged anywhere.
- FR-27: Masking MUST err toward removing too much rather than too little.
- FR-26a: Masking is **not configurable** and has no opt-out. App access is not
  equivalent to shell access on these hosts — the UI is reachable through a
  public tunnel — so a switch that disables masking is a switch that enables a
  disclosure. (C1)
- FR-28: Requests for a node's metrics MUST be authenticated by the same
  mechanism that already protects communication between cluster machines. A
  captured request MUST NOT be usable against a *different* target — in
  particular, a signature MUST cover the requested cursor, so it cannot be
  replayed to fetch a different range.
- FR-28a: Verbatim replay within the existing timestamp-drift window is **not**
  prevented, because the shared transport does not carry a nonce for bodyless
  requests — the same is already true of every other read-only cluster endpoint.
  Replaying a metrics fetch yields only data the holder of a valid signature
  could fetch anyway. Closing this would mean adding a nonce to transport code
  shared by every worker route, which is not justified by this feature. Recorded
  as an accepted residual in `analysis.md` E2, with the condition that reopens
  it. (analysis E2)
- FR-29: The browser MUST NOT be able to influence what is read from a host —
  no file path, process identifier, or filter supplied by the client.

## Out of Scope

- Any service other than Vibe Kanban.
- Any deployment or infrastructure change. This feature adds no new network
  port, service, or configuration option.
- Acting on a process or a host: no terminate, kill, renice, or restart.
- Historical storage beyond the short in-memory window: no time-series database,
  no metrics on disk, no retention across a restart.
- Alerting, thresholds, or anything that notifies an operator.
- Changing scheduling, placement, lease, or health semantics.
- GPU metrics, per-process disk I/O, and container or cgroup breakdowns.
- Collection on non-Linux hosts. Such a node reports "unsupported"; the cluster
  is Linux-only and the one macOS machine is a client, not a node.
- **Mobile viewports.** The view is desktop-only: a 420–720px panel of dense
  numeric readouts has nowhere to go on a phone, and mounting its live
  subscription there would cost bandwidth and battery for something unreadable.
  FR-8's "anywhere in the application" is scoped to desktop layouts. (analysis
  W8)

## Acceptance Criteria

- [ ] With clustering disabled, opening the view shows exactly one node — the
      coordinator — with live CPU, memory, disk, network, and process readings.
- [ ] With clustering enabled and both workers registered, the view lists three
      nodes, and selecting each shows that host's own readings, not another's.
- [ ] Per-core CPU readings on a node agree with `btop` running on that same
      host to within a few percentage points.
- [ ] The first readings after startup show rate-derived values as
      not-yet-available, not as zero and not as a spike.
- [ ] Running a CPU burner on one worker moves that worker's meters and leaves
      the other nodes' meters unchanged.
- [ ] Filling a filesystem on a node is reflected in that node's disk panel,
      and the shared NFS mount appears among the listed filesystems.
- [ ] Stopping a worker's service shows that node as unreachable within a few
      seconds, while that worker's recorded status, lease, and eligibility to
      receive work are unchanged, and a workspace already running there is
      unaffected.
- [ ] A worker running a build that predates this feature is shown as
      unsupported-by-that-version, and the other nodes continue to report.
- [ ] Closing the view stops all collection from remote nodes — verified by the
      workers' access logs going quiet.
- [ ] A process whose command line contains an API key, a password in a URL, or
      a bare access token appears in the process list with that value masked,
      and the unmasked value appears in no log or stored record.
- [ ] A request for a node's metrics that is unsigned, carries a stale
      timestamp, or reuses a signature against a different cursor is refused.
- [ ] Node health shown in the panel matches Settings for a worker whose lease
      has expired — and building the panel does **not** change that worker's
      stored status or `updated_at`.
- [ ] With clustering disabled (no coordinator id configured), the single
      coordinator node keeps the same identifier across a restart, so a
      persisted node selection still resolves.
- [ ] A worker registering or deregistering while the view is open does not
      corrupt the displayed readings for any other node.
- [ ] One node returning malformed data does not blank the view.
- [ ] Panel open state, width, selected node, and expanded sections are the same
      after a page reload.
- [ ] Node health shown here matches node health shown in Settings, including
      for a worker whose lease has expired.
- [ ] The panel can be dismissed from the keyboard, focus returns to the control
      that opened it, and every meter exposes its value as text.

## Open Questions

None. All five original markers were resolved in
[`clarifications.md`](clarifications.md); the resulting decisions are folded
into FR-6a, FR-9a, FR-15a, FR-18, and FR-26a above.

One item is deliberately out of scope rather than open: the two inconsistent
React Query keys for worker nodes elsewhere in the app. Consolidating them is
unrelated to this feature and would widen its blast radius into the
workspace-placement UI.
