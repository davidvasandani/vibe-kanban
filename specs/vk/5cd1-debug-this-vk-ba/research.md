# Research: Claude hook control-stream closure

## Local history

- `e072e906` fixed late Stop-hook failures by adding
  `POST_RESULT_GRACE = 500 ms` and keeping the reader alive during it. Later
  changes also ignored a specific non-error `num_turns=0` resume artifact.
- `79941275` introduced the parameter-level background Bash hook. Its tests
  cover matchers, predicate conservatism, permission modes, and response JSON,
  but not child-pipe lifetime.
- Current branch code arms the grace deadline with `get_or_insert_with`.
  Receiving later lines does not refresh it.

## Upstream evidence

- Anthropic TypeScript SDK issue #369 documents the same error string when
  shared stdin closes on the first result while later hook/canUseTool
  round-trips still need it.
- Anthropic TypeScript SDK issue #376 extends the case to background subagent
  activity after a top-level result.
- Anthropic Python SDK issue #730 documents a timer closing a bidirectional
  stream mid-conversation and producing `Tool permission stream closed before
  response received`.

The VK adapter does not use those SDK wrappers, but it implements the same
structured-input lifecycle directly. The upstream findings establish that a
result frame is not, by itself, proof that no more hook traffic exists.

## Pinned artifact

The task branch pins Claude Code 2.1.200; current `origin/main` pins 2.1.268.
Implementation and live reproduction must use the current base. The hook ID is
VK-defined and `Bash` plus `run_in_background` were already verified against
the executable artifact by the poller task. This change does not invent or
rename a vendor identifier.

## Alternatives considered

### Increase the grace constant

Rejected as the primary fix. It makes the race less likely but retains an
absolute deadline that can expire while activity is visibly continuing.

### Wait forever after a result

Rejected. The structured-input CLI may wait for stdin EOF, producing a lifecycle
deadlock for every normal completed turn.

### Remove the background-Bash hook

Rejected. Tool-name denial cannot close the actual
`Bash(run_in_background=true)` path, and background descendants are reaped at
turn end.

### Disallow all Bash

Rejected. Bash is a core foreground tool; this would be a severe regression.

### Treat only parsed control messages as activity

Rejected. Assistant, progress, and unknown future message types can separate a
result from later control traffic. The defensive protocol contract requires
unknown events to degrade safely, so every non-empty line refreshes quiescence.

## Decision

Use a resettable post-result idle deadline and deterministic duplex tests. This
turns the existing grace into a quiescence detector without removing the
bounded-exit guarantee.
