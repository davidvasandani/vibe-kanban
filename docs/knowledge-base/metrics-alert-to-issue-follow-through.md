# Metrics alert to issue follow-through

Tags: `vk/32f3-server-metrics-w`, `vk/9a0b-boot-disk-warnin`

## Keep alert policy server-owned

Expose effective thresholds in the authoritative metrics snapshot and include
them in full resnapshots. The browser may carry documented defaults only for
rolling-version compatibility. Parse deployment overrides by distinguishing an
unset variable from a present-but-invalid value; reject the latter visibly and
fall back to the complete default policy.

For low disk, the conservative rule is an OR: alert when either free percentage
or free bytes is below its boundary. Validate percentages in 0..=100, severity
ordering, and filesystem facts (`used <= total`, `available <= total`) before
classification. Retained readings from unavailable nodes must not alert; an
explicit stale reading may remain a timestamped warning but must not retain a
critical claim.

Telemetry inclusion and alert eligibility are separate decisions. Keep samples
available for inspection even when they are not meaningful inputs to a
workload-oriented alert. In particular, small dedicated boot mounts (`/boot`
and descendants) can sit below an absolute free-byte threshold by design while
having ample proportional headroom; exclude them at the shared classification
boundary instead of weakening the percentage-or-bytes OR rule or filtering the
metrics payload. Match mount paths on segment boundaries so an unrelated name
such as `/bootstrap` remains eligible.

## A collapsed rollup owns a bounded snapshot query

An accordion body can stay unmounted while collapsed to avoid retaining its
WebSocket. A compact header rollup therefore needs its own host-scoped, bounded
REST query. Use the same query key as the expanded view so cached snapshots are
shared, and route both reads and actions through the selected host. Otherwise a
remote-host warning can be validated against the wrong coordinator.

## Re-resolve evidence at the coordinator

The warning action should send identity and intent, not trusted facts. The local
coordinator reloads the current snapshot, verifies node availability and valid
filesystem capacities, re-applies the effective warning threshold, and replaces
the submitted hostname, timestamp, and filesystem list with canonical evidence
before forwarding the mutation.

## Durable duplicate-safe issue resolution

Persist a machine-readable `(kind, node_id)` identity in issue metadata and scope
the open key by project. Serialize concurrent resolutions with a transaction
advisory lock, find an existing nonterminal issue, or create one in the first
visible nonterminal status. In this schema terminality is a completion timestamp
or the established case-insensitive `Done`, `Cancelled`, or `Canceled` names;
project statuses do not have a terminal flag.

Return the issue, `created`, and the transaction ID. A reused issue needs an
observable no-op row update in the same transaction so Electric can see its txid;
a read-only transaction ID never appears in the shape stream. The client can then
await convergence for both create and reuse before navigation. Use an immediate
ref as well as rendered disabled state to suppress same-tick double activation;
server-side locking remains the durable invariant.

## Deployment boundary

Service code reads environment variables, while the homelab module owns their
operator-facing names, units, defaults, and assertions. Keep these definitions
aligned and test the evaluated Nix option values alongside Rust validation.
