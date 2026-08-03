# Contracts: Gmail MCP connector (`vk/4daf-gmail-mcp`)

No HTTP API changes. No new Rust types, so `shared/types.ts` is untouched and
`generate-types` is not run. Two contracts are worth pinning down because tests
assert them.

---

## C-1. `nextAvailableServerName` (internal, TypeScript)

**Location**: `packages/web-core/src/shared/lib/sharedMcpSettingsState.ts`

```ts
export function nextAvailableServerName(
  key: string,
  existing: readonly string[]
): string;
```

**Purpose.** Allocate a logical MCP server identifier for a newly instantiated
catalog template, given the identifiers already present in the draft.

**Behaviour**

| `key` | `existing` | Returns |
| --- | --- | --- |
| `gmail` | `[]` | `gmail` |
| `gmail` | `['slack', 'context7']` | `gmail` |
| `gmail` | `['gmail']` | `gmail_2` |
| `gmail` | `['gmail', 'gmail_2']` | `gmail_3` |
| `gmail` | `['gmail', 'gmail_3']` | `gmail_2` |

**Invariants**

1. **Valid by construction.** For any `key` matching `^[a-zA-Z0-9_-]+$`, the
   result matches `^[a-zA-Z0-9_-]+$`. This is the binding to the backend
   validator `is_valid_server_identifier`
   (`crates/executors/src/shared_mcp_config.rs:208`). Constitution XXII requires
   it, and a test asserts it rather than leaving it to inspection.
2. **Never collides.** The result is not in `existing`. The backend rejects
   duplicates (`validate_write_request`, `:928`), and — worse — the frontend's
   `setServer` de-duplicates by name (`McpSettingsSection.tsx:536-548`), so a
   colliding name silently **overwrites** the existing server instead of
   erroring.
3. **Pure and deterministic.** No clock, no randomness, no module state; the same
   arguments always give the same result. It fills gaps rather than counting
   instances, which is why case 5 above returns `gmail_2` and not `gmail_4`.
4. **Separator is `_`.** Not a space, not `(n)`. Those are rejected by the
   validator or silently rewritten by `suggested_server_identifier`.

**Caller contract.** `addPreconfigured`
(`McpSettingsSection.tsx:609-627`) must pass the names from the current
**draft** — the same source the tile's `added` flag reads — so that an unsaved
instance still reserves its name. As a `useCallback`, it must list
`draft.servers` in its dependency array (or use the functional update form), or
it closes over a stale list and hands out one name twice.

---

## C-2. Catalog entry shape (`gmail`)

**Location**: `crates/executors/default_mcp.json`

```json
"gmail": {
  "command": "npx",
  "args": [
    "-y",
    "github:davidvasandani/Gmail-MCP-Server#030da3492753222a41645a9f343466d151c63f3c",
    "--tool-prefix=YOUR_TOOL_PREFIX"
  ],
  "env": { "GMAIL_CREDENTIALS_PATH": "YOUR_CREDENTIALS_PATH" }
},
"meta": {
  "gmail": {
    "name": "Gmail",
    "description": "Read, search, draft and send Gmail from your agent",
    "url": "https://github.com/davidvasandani/Gmail-MCP-Server"
  }
}
```

**Asserted invariants** (`crates/executors/src/mcp_config.rs`, `mod tests`)

1. **Transport-neutral.** `command` / `args` / `env` only. Per-agent shape is the
   adapter's job — a repository constraint in the constitution.
2. **Immutable, content-addressed source.** The install spec's commit-ish equals
   the `GMAIL_MCP_FORK_REVISION` constant and is 40 lowercase hex characters. The
   spec contains none of `#main`, `#master`, `refs/heads/`, `@latest`, and is not
   a fragment-less bare repo reference.
3. **One claim, not two.** The `owner/repo` in the install spec equals the
   `owner/repo` in `meta.gmail.url`. `meta.<server>.url` is a link shown in the
   UI and has no effect on what is installed, so the agreement must be asserted
   or it is not guaranteed.
4. **Placeholders, never secrets.** Every value a user must supply is a `YOUR_*`
   placeholder. Nothing in the codebase validates that placeholders were
   replaced.
5. **Shared values are not placeholders.** `GMAIL_OAUTH_PATH` is absent: the
   OAuth client is per Google Cloud project, not per mailbox, so its default is
   correct and shared across a user's instances.
6. **Survives adaptation.** Under the Codex adapter the entry retains
   `command`/`args`/`env`; under the Opencode adapter it becomes
   `type: "local"` with `command` as an **array** and the environment map renamed
   to `environment` (`mcp_config.rs:480`). Dropping that rename makes
   credential-dependent entries unusable, so it is pinned by test.

**Not asserted, deliberately**: a SHA-256 digest of the artifact. A git commit
SHA is content-addressed and verified by npm at install time; see `research.md`
R3 and Constitution XVI.

---

## C-3. Tile behaviour (no code contract, but test-visible)

`addPreconfigured` produces a **new** logical server on every invocation. The
catalog tile's `added` flag continues to drive the check mark and dimmed styling
but no longer sets `disabled`, so a template can be instantiated repeatedly.

Asserted by: "adding a template twice yields two distinct draft servers"
(`sharedMcpSettingsState.test.ts`).
