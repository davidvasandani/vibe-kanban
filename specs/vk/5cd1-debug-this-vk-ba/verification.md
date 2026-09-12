# Verification: reliable Claude background-Bash denial

## Root-cause reproduction

On current `origin/main` (Claude Code 2.1.268), the new duplex transport test
was first run with the existing absolute post-result deadline:

```text
protocol input remains open after post-result activity: Kind(BrokenPipe)
test ...post_result_activity_keeps_background_bash_hook_stream_open ... FAILED
```

The test sends a real terminal result, waits 350 ms, sends another structured
output line, waits 250 ms, and then sends
`DENY_BACKGROUND_BASH_CALLBACK_ID`. The hook is only 250 ms after the latest
activity, but 600 ms after the result. The old loop closed at 500 ms, proving
that its absolute timer—not the callback payload—caused the stream failure.

## Implemented behavior

- `ProtocolPeer` now treats `POST_RESULT_GRACE` as a quiescence interval.
  Every non-empty line after a result resets the deadline.
- A parsed control request is still handled inline and its response flushed
  before the loop checks the timer again.
- Hook/permission response write errors now fail the reader loop rather than
  being logged and discarded.
- A private generic async-I/O seam allows duplex transport tests while the
  production constructor still accepts Claude child stdin/stdout.

## Focused tests

```text
cargo test -p executors executors::claude::protocol::tests -- --nocapture --test-threads=1
3 passed; 0 failed
```

The module tests cover the reported late background hook, true quiescent
shutdown, and response-stream write failure.

## Broader executor verification

```text
cargo fmt --all -- --check
cargo test -p executors
282 passed; 0 failed; 1 ignored
doc tests: 0 passed; 0 failed; 1 ignored
```

## Delivery

- Pull request: https://github.com/davidvasandani/vibe-kanban/pull/280
