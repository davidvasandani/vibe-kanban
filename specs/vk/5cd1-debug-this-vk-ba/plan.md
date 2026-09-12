# Implementation Plan: reliable Claude background-Bash denial

**Spec**: `./spec.md`
**Status**: Ready for tasks

## Technical context

The change is confined to the Rust `executors` crate. Claude Code runs as a
natural-exit child with piped stdin/stdout. `ProtocolPeer` owns the
bidirectional structured-I/O stream: it writes initialization, permission mode,
prompts, interrupts, and hook responses to stdin while reading CLI messages
from stdout.

After a terminal `result`, the loop currently arms a one-shot 500 ms deadline.
It handles messages that arrive inside that window but never moves the
deadline. Consequently, continued output can prove the session is active while
the old timer still closes stdin immediately afterward. A subsequent
`DENY_BACKGROUND_BASH_CALLBACK_ID` request fails inside Claude with
`Stream closed`.

## Architecture and approach

1. Update the task branch to current `origin/main`, retaining the SpecKit
   artifacts.
2. Generalize `ProtocolPeer`'s private I/O storage/read-loop bounds from
   concrete child pipe types to Tokio async I/O traits only as far as needed for
   a deterministic duplex test. Keep the public production constructor simple.
3. Replace “500 ms after first terminal result” with “500 ms of quiescence after
   terminal result”:
   - arm the deadline on the first real terminal result;
   - whenever any subsequent non-empty stdout line is received, move the
     deadline to `now + POST_RESULT_GRACE`;
   - parse and handle a control request inline, flushing its response before
     selecting the timer again;
   - continue to ignore the known non-error zero-turn resume artifact and retain
     its separate bounded fallback.
4. Make the private loop return response-write failures where doing so does not
   change cancellation semantics, instead of treating a failed mandatory
   callback response as success.
5. Add paused-time duplex tests proving:
   - a result followed by activity near the old deadline and then a background
     hook request receives the deny response;
   - true quiescence closes the loop within the existing bound;
   - foreground/background callback value semantics remain unchanged.

## Data model

No persisted or API data changes. See `./data-model.md`.

## Contracts

The internal Claude structured-I/O sequencing contract is recorded in
`./contracts.md`. There are no public REST, MCP, or generated-TypeScript
contract changes.

## Research notes

See `./research.md` for source history, pinned-artifact considerations, and
rejected alternatives.

## Constitution check

- **II, Test the contract:** adds a transport-level timing regression rather
  than relying only on JSON-value tests.
- **III and VI, Small/reuse:** adjusts the existing post-result grace mechanism
  rather than introducing a second protocol supervisor.
- **IX, External protocols:** preserves verified hook identifiers, unknown
  message tolerance, cancellation, and fail-loud denial behavior.
- **XII, Async handoffs:** a received callback remains owned through response
  flush; the idle timer measures absence of activity rather than bypassing an
  active handoff.
- **XXI, Failures identify the fact:** mandatory response-write errors remain
  visible as protocol failures.

No constitution deviations are required.

## Risks and dependencies

- A too-eager definition of “activity” could still close between related
  messages. Counting every non-empty stdout line is intentionally conservative.
- A malformed/noisy CLI could extend shutdown by repeatedly writing. Each
  extension remains bounded from the latest activity, and the child/process
  cancellation path remains available.
- Refactoring I/O types can create unnecessary generic complexity. Limit the
  abstraction to an internal generic loop or boxed trait object and keep child
  spawn signatures unchanged.
- The branch predates current main and must be merged before code edits to avoid
  testing the old Claude 2.1.200 integration instead of the current pin.
