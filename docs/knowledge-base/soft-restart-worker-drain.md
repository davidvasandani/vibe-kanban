# Soft restart worker drain and browser recovery

Tags: `vk/9632-vk-soft-restarts`

## Reuse the process owner that already survives

In clustered production, the worker—not the coordinator HTTP server—owns coding
agent process groups, input, ordered output journals, exit observation, and
cancellation. Coordinator replacement is therefore already structurally safe:
worker registration retries, reconciliation runs before orphan cleanup, and
worker-owned running rows are excluded from local orphan recovery. Prefer
hardening this boundary over adding a second supervisor.

This does not make the worker self-updating. Replacing its binary still replaces
the process owner and kills its children, so release activation needs an
evidence-backed idle handoff.

## Drain before replacing the worker

The safe worker activation sequence is:

1. persist an admission-drain marker and close new execution admission;
2. observe drain acknowledgement through health;
3. wait for authoritative active execution count to reach zero;
4. replace and health-check the worker while the candidate starts drained;
5. clear the marker and reopen admission only after success or rollback.

Same-ID/same-digest retries remain admissible during drain because they do not
create work. Count in-progress admissions to close the check-then-insert race.
Count actual live child ownership as well as nonterminal protocol states:
reconciliation may quarantine a record without stopping its process. If
liveness cannot be observed, fail closed and defer activation.

Persisted state makes signal delivery an instruction rather than the only copy
of the drain decision. Send systemd signals to `--kill-whom=main`; service-level
signals may otherwise reach agent descendants in the same cgroup.

## Bootstrap and shell parsing traps

Workers predating the health contract cannot acknowledge a race-free drain.
Defer their first upgrade and require a one-time operator-confirmed idle
activation. Once the new worker is installed, subsequent upgrades use the
automatic protocol.

When detecting a JSON boolean capability with `jq`, use `has("field")`.
Expressions such as `.field // empty` treat the valid value `false` as absent
and can incorrectly route a new, idle worker through the legacy bootstrap path.

## Keep browser state across transport replacement

A WebSocket cleanup caused by a same-endpoint retry must close handlers and the
socket without clearing the last initialized snapshot. Reset snapshot and
initialization only when the endpoint or enabled identity changes. Reconnect
with bounded exponential backoff and jitter, and expose connection state as an
additive status banner after initial load; the existing workspace view remains
rendered during the outage.

## Explicit limitation

This pattern preserves cluster-worker-owned coding agents. It does not preserve
standalone/local-server children or PTY sessions. True PTY continuity requires
stable ownership plus an acknowledged input/output journal and replay contract;
keeping only a socket or child handle is not sufficient.
