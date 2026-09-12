# Contract: Worker metrics endpoint

The coordinator→worker pull channel. One new route on the worker's existing
signed `/v1/*` router.

## `GET /v1/metrics?after={u64}`

Added to the router in `crates/worker/src/worker_api.rs:89-114`, **inside** the
`require_signature` layer, so it inherits the existing transport authentication
unchanged.

### Request

| Element | Value |
| --- | --- |
| Method | `GET` |
| Path | `/v1/metrics` |
| Query | `after` — required, `u64`. Return only samples with `sequence > after`. `after=0` returns the whole retained ring. |
| Body | None |

Headers, unchanged from every other `/v1/*` call:

| Header | Value |
| --- | --- |
| `x-vk-timestamp` | **Unix epoch seconds as a decimal string** (not RFC3339 — the worker does `.parse::<i64>()` at `worker_api.rs:416-420`, and the client emits `Utc::now().timestamp().to_string()` at `client.rs:346`). Must be within ±30s of the worker's clock |
| `x-vk-content-sha256` | `base64(sha256(""))` — the digest of the empty body |
| `x-vk-signature` | ed25519 over the canonical string below, by the coordinator key |

Canonical signed string (`worker_api.rs:449-454`):

```
{timestamp}.{METHOD}.{path_and_query}.{base64(sha256(body))}
```

**`path_and_query` includes `?after=N`.** This is load-bearing: without it, a
captured signature could be reused against any cursor. The same rule already
protects `/v1/executions/{id}/events?after=N`.

Like `GET /v1/jobs`, this route carries **no** payload-level `RequestAuthority`
— there is no body to bind one to. Transport signature plus timestamp drift is
the whole authentication story.

**There is no nonce check on this route.** `require_signature`
(`worker_api.rs:409-463`) validates only the timestamp, the signature, and the
body digest; the nonce map is consulted exclusively by `validate_authority`
(`:386-407`), which runs for body-carrying routes. A captured request is
therefore replayable verbatim for up to 30 seconds — as is already true of
`/v1/jobs`, terminal output, and event fetches. See FR-28a and analysis E2 for
why this is accepted rather than fixed here.

### Response `200`

`SampleBatch`, per `../data-model.md`:

```json
{
  "samples": [
    {
      "sequence": 412,
      "hostname": "think2",
      "captured_at": "2026-08-01T04:32:18.114Z",
      "interval_ms": 2003,
      "uptime_seconds": 15537447,
      "cpu": {
        "model": "Intel(R) Core(TM) i5-8500T CPU @ 2.10GHz",
        "core_count": 6,
        "total_busy_percent": 54.0,
        "per_core_busy_percent": [53.0, 52.0, 56.0, 54.0, 53.0, 57.0],
        "load_1m": 3.61, "load_5m": 3.17, "load_15m": 3.19,
        "frequency_mhz": 3200,
        "temperature_celsius": 70.0
      },
      "memory": {
        "total_bytes": 67222241280, "available_bytes": 61724278784,
        "used_bytes": 5497962496, "cached_bytes": 17609365504,
        "swap_total_bytes": 0, "swap_used_bytes": 0
      },
      "filesystems": [
        { "mount_point": "/", "device": "/dev/dm-0", "fs_type": "ext4",
          "total_bytes": 3936121651200, "used_bytes": 990098128896,
          "available_bytes": 2936023522304 }
      ],
      "networks": [
        { "interface": "enp1s0",
          "rx_bytes_total": 2408088502272, "tx_bytes_total": 19026383241216,
          "rx_bytes_per_second": 143360, "tx_bytes_per_second": 303104 }
      ],
      "processes": [
        { "pid": 2916008, "start_ticks": 8837412, "name": "vibe-kanban",
          "user": "vibe-kanban", "command": "/srv/vk-releases/current/bin/vibe-kanban",
          "cpu_percent": 144.0, "memory_bytes": 1073741824, "thread_count": 139 }
      ],
      "degraded": []
    }
  ],
  "earliest_retained_sequence": 263,
  "latest_sequence": 412
}
```

Contract details:

- `samples` is ordered oldest → newest and contains only `sequence > after`. It
  may be **empty** (nothing new since the cursor), hold one entry, or hold up to
  the full retention window when `after=0`. The sampler and the poller are
  independently phased, so a nominal 2s poll routinely returns 0 or 2 samples.
- **`processes` is populated on the newest sample only.** Older samples in the
  batch carry `null` — not `[]`, which would be indistinguishable from "no
  processes were readable". See clarification C4: the table is ~80% of a sample
  and nothing plots it over time.
- Rate-derived fields (`total_busy_percent`, `per_core_busy_percent`,
  `*_bytes_per_second`, `cpu_percent`) are `null` when no predecessor existed —
  never `0` (FR-7).
- `earliest_retained_sequence` lets the caller detect that its cursor fell out
  of the ring. For metrics that is benign; the caller records the discontinuity
  and forces a resnapshot rather than erroring.
- `command` is **already redacted** when it arrives. Redaction happens inside
  the sampler on the worker, so an unredacted command line never crosses this
  wire. The coordinator does no further masking and must not assume it needs to.
- Absent readings are `null`, never `0`.
- Every float lives in a plain struct. No float appears inside a tagged enum
  (the `preserve_order` hazard).

### Response `401`

Returned by `require_signature` before the handler runs, for: a missing or
invalid signature; a `x-vk-timestamp` outside ±30s; a body digest mismatch; or a
signature computed over a different `path_and_query` — including a different
`after` value.

### Response `404`

Returned by a worker whose build predates this feature. The coordinator maps
this to `NodeMetricsAvailability::NotImplemented`, distinct from `Unreachable`,
logs it at most once per node, and continues serving every other node.

### Response size

Bounded by construction: `retention` (150) × sample size, with the process table
present once. On a 6-core node with 6 filesystems this is roughly 150 KB for a
full `after=0` fetch, and a few KB for the steady-state `after=N` case.

The coordinator applies an explicit response cap before buffering and treats an
oversized reply as `Unreachable { reason }` for that node only.

## Coordinator client

`WorkerClient::metrics(worker_node_id, after) -> Result<SampleBatch, WorkerClientError>`,
added beside `inventory()` in
`crates/services/src/services/cluster/client.rs:279-290`, which is the same
shape (signed GET, no body).

Differences from `inventory()`:

| Aspect | Value | Why |
| --- | --- | --- |
| Timeout | 5s per request | The client default is 30s (`client.rs:63`), far too long for a 2s tick — a hung node would stall every subsequent poll |
| Envelope | Fresh timestamp per call | A cached envelope falls outside the ±30s drift window within half a minute and starts failing |
| Concurrency | One in-flight request per node | Prevents pile-up if a node is slow but not timing out |
| 404 handling | Mapped to a distinct error variant | So the caller can report `NotImplemented` rather than `Unreachable` |

## What this contract does *not* do

- It does not change `ResourceSnapshot`, `WorkerHeartbeat`, or
  `PROTOCOL_VERSION`. The heartbeat continues to carry the four scheduler
  scalars under their existing key names.
- It does not let the caller influence what is read. There is no path, pid,
  filter, or count parameter — only a cursor. Everything the sampler reads is
  compiled in.
- It never causes a worker to be marked offline, drained, or ineligible.
