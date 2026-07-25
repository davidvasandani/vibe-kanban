# Contracts: pinned Slack MCP connector from the maintained fork

## 1. Catalog entry (`crates/executors/default_mcp.json`)

Canonical, transport-neutral form:

```json
"slack": {
  "command": "npx",
  "args": [
    "-y",
    "https://github.com/davidvasandani/slack-mcp-server/releases/download/v1.3.0-vk.2/slack-mcp-server-vk-1.3.0-vk.2.tgz",
    "--transport",
    "stdio"
  ],
  "env": { "SLACK_MCP_XOXP_TOKEN": "YOUR_TOKEN" }
}
```

Invariants asserted by tests in `crates/executors/src/mcp_config.rs`:

| Invariant | Assertion |
| --- | --- |
| `command == "npx"` | exact match |
| `args[0] == "-y"`, `args[2..] == ["--transport", "stdio"]` | exact match |
| `args[1]` matches `https://github.com/{owner}/{repo}/releases/download/{tag}/{asset}` | shape parse |
| `{owner}/{repo}` == owner/repo of `meta.slack.url` | cross-field equality |
| `{tag}` == `SLACK_MCP_FORK_TAG` | constant equality |
| `args[1]` contains none of `@latest`, `#master`, `refs/heads/`, `/archive/` | negative match |
| `env == {"SLACK_MCP_XOXP_TOKEN": "YOUR_TOKEN"}` | exact match |
| SHA-256 of the fetched `args[1]` == `SLACK_MCP_LAUNCHER_SHA256` | `#[ignore]`d network test |

Per-agent adaptations (unchanged behaviour, re-asserted):

- **Codex**: `command: "npx"`, `args` as above, `env` retained.
- **Opencode**: `type: "local"`, `command: ["npx", "-y", "<url>", "--transport", "stdio"]`,
  environment under the `environment` key.

## 2. Launcher CLI contract (`slack-mcp-server-vk-<version>.tgz`)

**Invocation**: `slack-mcp-server [...args]` — every argument is forwarded to the
Go binary verbatim; the launcher parses none of them.

**Resolution order**

1. `SLACK_MCP_SERVER_VK_BINARY` set → exec that path (no download, no digest
   check). Missing/not executable → exit 1 with a diagnostic naming the variable.
2. Cached at `<cache-root>/slack-mcp-server-vk/<version>/<asset>` and matching
   the embedded digest → exec it.
3. Otherwise download `<release-url>/<asset>`, verify SHA-256, `chmod 0755`,
   atomically rename into the cache, exec it.

`<cache-root>` = `%LOCALAPPDATA%` on Windows, else `$XDG_CACHE_HOME`, else
`~/.cache`; `SLACK_MCP_SERVER_VK_CACHE_DIR` overrides.

**Platform map** (`process.platform`-`process.arch` → asset)

| Node platform/arch | Asset |
| --- | --- |
| `darwin-x64` / `darwin-arm64` | `slack-mcp-server-darwin-amd64` / `-arm64` |
| `linux-x64` / `linux-arm64` | `slack-mcp-server-linux-amd64` / `-arm64` |
| `win32-x64` / `win32-arm64` | `slack-mcp-server-windows-amd64.exe` / `-arm64.exe` |

Anything else → exit 1, `unsupported platform: <platform>-<arch>`.

**Stdio and lifecycle**

- Child inherits stdin/stdout/stderr — the launcher never reads, writes,
  buffers, or transforms the MCP byte stream.
- Launcher diagnostics go to **stderr only**, one line each, prefixed
  `slack-mcp-server-vk:`; stdout stays pure JSON-RPC.
- SIGINT / SIGTERM / SIGHUP are forwarded to the child.
- Parent exits with the child's exit code; if the child died from a signal, the
  parent re-raises that signal after restoring the default handler.

**Failure modes** (all exit non-zero, none fall back to another build)

| Condition | Message shape |
| --- | --- |
| Unsupported platform | `unsupported platform: <p>-<a>` |
| Download failed (HTTP status / network) | `download failed for <asset>: <reason>` |
| Digest mismatch | `checksum mismatch for <asset>: expected <e>, got <g>` |
| Missing digest entry for the asset | `no recorded checksum for <asset>` |
| Explicit binary missing | `SLACK_MCP_SERVER_VK_BINARY=<path> is not executable` |

## 3. Release artifact contract (tag `v1.3.0-vk.2`)

| Asset | Notes |
| --- | --- |
| `slack-mcp-server-{darwin,linux,windows}-{amd64,arm64}[.exe]` | `make build-all-platforms` at the tag; `--version` reports `v1.3.0-vk.2` |
| `slack-mcp-server-vk-1.3.0-vk.2.tgz` | launcher, `npm pack` output, digests embedded |
| `checksums.txt` | `sha256sum` lines for every asset above |

Immutable: assets are never re-uploaded under an existing tag; corrections ship
as `v<base>-vk.<n+1>`.

## 4. MCP tool surface (inherited from the fork, re-asserted by verification)

- `tools/list` includes `attachment_get_data` by default; it is absent only when
  `SLACK_MCP_ENABLED_TOOLS` is set to a list that omits it.
- `attachment_get_data(file_id: "F…")` → file metadata + content (text as-is,
  binary base64), 5 MB cap. The argument is `file_id`; a missing or malformed
  one is rejected by the handler itself (`file_id is required`, `invalid file_id
  format`), which is also how a caller can tell the tool exists.
- Slack-origin failures map to actionable messages (`missing_scope`,
  `not_authed`/`invalid_auth`, `access_denied`, `file_not_found`,
  `file_deleted`); VK adds no bypass of Slack's channel/file permissions.
