# Agent-facing fail-loud boundaries

Tags: `vk/94c0-three-loose-ends`

## Typed errors are not the non-browser contract

A route can return the correct typed error and still be unusable through MCP.
`ApiResponse::error_with_data` intentionally leaves `message` empty, while the
task-server client surfaces `message` and falls back to `Unknown error`. Frontend
tests that match `error_data` therefore do not cover what an agent receives.

For MCP-reachable validation failures, preserve both channels: typed error data
for browser behavior and an actionable message for non-browser callers. Map
every variant to a message that names the failed rule and tells the caller how to
correct it. The regression test constructs the response envelope and asserts its
message, rather than stopping at the internal enum.

## Plausible vendor config is not a control

Generic config maps make removed protocol fields deceptively easy to retain. A
field can disappear from a typed app-server request, be moved into a string-keyed
map, pass serialization and review, and then be silently ignored by the vendor.
Trace its history and check the source for the exact pinned executable before
inventing a replacement.

When the vendor supplies strict config validation, enable it at the process
boundary and pin the exact launch flag in a built-command test. Also assert the
specific safety key remains present: strict validation proves that a key is
recognized, while the focused assertion proves the desired value is emitted.
Dead public settings should be removed from their Rust source and regenerated
contracts through the repository generator, not left as inert compatibility
theater.
