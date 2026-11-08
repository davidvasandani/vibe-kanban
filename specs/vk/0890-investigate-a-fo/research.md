# Research
The existing wiki explicitly identifies the user-follow-up versus comment dispatcher race.
The direct follow_up route performs container setup and session reads before creating its
execution and has no running-turn check. start_execution captures repository state before
creating its Running row. Worker dispatch deduplicates execution ID only, under a jobs lock.
These are source findings; the supplied historical incident is not independently revalidated.

The existing Claude integration enforces unsupported tool restrictions through both CLI
arguments and PreToolUse callbacks. Reuse it for native ListAgents/SendMessage rather than
assuming TMPDIR redirects a hard-coded /tmp/cc-socks directory.

## Pinned Claude artifact verification (2026-11-08)

Inspected the bundled JS strings in the Nix Claude 2.1.281 native artifact
`/nix/store/r331x6dfyzn5k8mj74p16gqm3ba1aw4n-claude-code-2.1.281/bin/.claude-unwrapped`,
matching the repository pin. Its alias map maps ListPeers to ListAgents; wire
constants are ListAgents and SendMessage. The peer list includes other local
sessions. The inbound framing still says peers are very likely working on the
user's behalf. The settings schema explicitly supports crossSessionInbound with
accept/hold/refuse and describes refuse as opting out of cross-session delivery.
The registry reads CLAUDE_CODE_SESSION_NAME and publishes name/nameSource;
`--name` is also supported. Socket creation prefers XDG_RUNTIME_DIR or its temp
root but falls back to /tmp/cc-socks-UID when the Unix socket path is too long.
Therefore setting TMPDIR alone is not a reliable isolation contract.

Use a private temporary merged settings file (preserving profile settings and
keeping contents out of argv), crossSessionInbound=refuse, both wire names plus
the ListPeers alias in the deny list/hooks, and stable vk/<workspace UUID>/<session UUID>
naming. CLI settings validation/init and mocked hook tests are separate from a
real receive-side delivery test; report their evidence separately.

## Incident evidence read from VK

`get_execution` confirms both execution IDs belong to session
615746b0-5a23-4297-9fc8-c25f135e1b37, with starts at
2026-10-31T15:27:26.454160727Z and 15:27:35.735875887Z (9.281715160 seconds apart).
The second action resumes Claude session 88ecb66f-e251-44f4-84a0-05b0a41f6323.
`search_session_messages(query=SendMessage)` scanned both turns completely and
returned three tool entries, including 15:54:32.704397051Z. The initial final
message explicitly describes the same-worktree collision and failed README edit.
Current completion timestamps are November 5; they differ from the incident
report and are not reliable evidence of the original October 31 exit times.
No unrelated service or historical workspace was modified.

The direct follow_up endpoint lacked a running check altogether, and MCP
run_session_prompt calls that endpoint unconditionally. This is sufficient to
explain overlapping executions even after the initial Running row exists; it
is broader than the hypothesized first-seconds race. The queue endpoint was a
separate path chosen by UI state, and comment delivery used a private lock.

A foreground native-CLI smoke test initialized the exact pinned binary using
crossSessionInbound=refuse, the deny list and stable name successfully, with no
model prompt or peer message sent; the test process was reaped. This validates
CLI acceptance, not an end-to-end blocked delivery between two live peers.
