# Internal contract: Claude structured-I/O shutdown

## Inputs from Claude stdout

- `control_request`: VK owns the request until it has written and flushed one
  matching `control_response`.
- `result`: arms terminal quiescence unless it is the known non-error
  zero-turn resume artifact.
- any other non-empty line after a terminal result: proves the stream is not
  quiescent and restarts the terminal idle interval.
- EOF: the child has closed output; the loop may finish.

## Outputs to Claude stdin

- Initialization, permission-mode changes, user messages, interrupts, and
  control responses remain newline-delimited JSON and are flushed after each
  message.
- A matching background-Bash hook response has
  `type=control_response`, the original `request_id`, subtype `success`, and
  a `PreToolUse` deny payload whose reason names `spawn_poller`.

## Shutdown

After a real terminal result, VK drops the structured input only when no
non-empty output has arrived for one complete `POST_RESULT_GRACE` interval.
A control request received before expiry is handled and its response flushed
before expiry is reconsidered. Cancellation before terminal result sends the
existing interrupt request. No public API contract changes.
