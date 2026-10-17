# Tests that fail only on the agent host

Some Rust tests read deployment environment variables. The VK agent host
sets those variables for real, so the tests fail there even on a clean
`main`. Before blaming a diff, re-run the failing test with the variable
unset.

| Failing tests | Host variable | Re-run with |
|---|---|---|
| `executors` `shared_mcp_config::tests::*slack*` (4 tests). Expected a stdio launcher, got `{"type":"http","url":…:13080/mcp}` | `VIBE_KANBAN_SLACK_MCP_URL` (routes Slack MCP to the shared HTTP gateway) | `env -u VIBE_KANBAN_SLACK_MCP_URL cargo test -p executors` |
| `worker` `execution::tests::dispatched_github_tokens_get_worker_local_routing`. The routed git config includes real owner helpers (`alderbridge`, `sweetgreen`, …) | `GIT_CONFIG_*`, `VK_GITHUB_PAT_*` (owner token routing, see [[github-owner-token-routing]]) | `env $(env \| grep -oE '^(GIT_CONFIG[A-Z_0-9]*\|VK_GITHUB_PAT[A-Z_0-9]*)' \| sed 's/^/-u /') cargo test -p worker` |

CI does not set these variables, so the tests are green there. Make the
tests hermetic only when you are working on them anyway, and do not hide
real failures behind a blanket `env -u`.

Running checks in a VK turn:
- A `cargo test -p executors -p worker` cold build can exceed the 10-minute
  tool timeout.
- `run_in_background` is blocked inside VK turns. Run the command in the
  foreground with `timeout`, or use `spawn_poller`.

## Contributed by

- `vk/2e22-disable-agent`
