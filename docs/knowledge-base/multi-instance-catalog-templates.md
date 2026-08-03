# Running one catalog template as several instances

Tags: `4daf-gmail-mcp`

How to let a bundled catalog entry (an MCP server template, but the shape
generalises) be instantiated more than once — one per account, workspace, or
credential set. Complements
[shared-mcp-configuration](shared-mcp-configuration.md) (the catalog contract)
and [forked-mcp-server-packaging](forked-mcp-server-packaging.md) (pinning).

## Why a template is not a singleton

A catalog entry describes a *kind* of server. Any tool whose credentials are
per-account needs one **process** per account, because the credential is loaded
once at startup — Gmail holds a single OAuth refresh token from a single
`GMAIL_CREDENTIALS_PATH`. So "connect two mailboxes" is not a feature of the
server; it is a client-configuration problem, and the settings screen is where it
has to be solved.

The backend already forces the conclusion: same name with different credentials
reconciles as a **conflict**, not as two servers
(`equivalent_slack_conflicts_on_semantic_stdio_differences` covers the
differing-token case). Per-account credentials therefore *require* per-account
names, mechanically. Multi-instance support is a prerequisite for this class of
connector, not a nicety.

## What blocked it, and the shape of the fix

Three things, all in `McpSettingsSection.tsx`, and the middle one is the
dangerous one:

- `addPreconfigured` used the catalog key verbatim as the logical server name.
- `setServer` **de-duplicates by name**: it filters same-name entries then
  appends. So a colliding name does not error — it silently replaces the earlier
  server.
- The tile set `disabled={added}` on an exact name match, so a second add was
  unreachable from the UI.

The fix is a pure allocator (`nextAvailableServerName`) returning `key`, else the
first free `key_2`, `key_3`, …, plus dropping `disabled`. Keep the "already
added" dimming — it is still true — but stop gating the action on it.

## Generated identifiers are valid by construction

These names are **protocol identifiers** written into agents' native config
files, not display labels. The backend validates `^[a-zA-Z0-9_-]+$`
(`is_valid_server_identifier`) and rejects duplicates outright.

So the separator is `_`, never a space or `(2)`. A frontend that invents a name
for a backend-validated field must derive it from that field's rule, and a test
must assert the generated form satisfies it. Discovering the mismatch at save
time — or relying on `suggested_server_identifier` to repair it behind the user's
back — is the defect the rule exists to prevent.

Corollary: there is **no display-label field**. `SharedMcpServer` carries only
`name`, and native agent configs store nothing but the map key, so "Gmail
(Personal)" cannot be stored. Adding a label means introducing the first
VK-owned MCP store; until then, users name instances `gmail_personal`.

## The taken-names set is not `draft.servers`

The bug worth remembering. `reconcile_snapshots` routes a name into `servers`
**XOR** `conflicts` — a name whose definitions diverge across agents appears
*only* in `conflicts`, and `draftFromSharedRead` preserves that split.

Allocating against `draft.servers` alone therefore hands out a name that may
already belong to an unresolved conflict. On save,
`plan_servers_for_executor` removes that name from every executor **not** in the
new server's assignments — and a freshly added template auto-assigns only one —
so the other agent's entry is deleted, with no conflict prompt and no mention in
the save summary.

Union both lists (`takenServerNames`) at every site that asks "is this name
free?" — allocation *and* the rename dialog's duplicate validation.

## Ship the collision disambiguator, don't leave it to be discovered

Some MCP clients dedupe tool entries by **base name across servers**. Two
instances of one server with unprefixed tools do not degrade gracefully: one
silently shadows the other, and the user sees a working `search_emails` reading
the wrong mailbox.

Where a tool offers a disambiguator (Gmail's `--tool-prefix` / 
`GMAIL_MCP_TOOL_PREFIX`), it belongs in the shipped entry as a required
placeholder. Two details that are easy to get wrong:

- **Put it wherever the tool gives it precedence.** Gmail's flag beats its env
  var, so the flag is the single authoritative source; carrying both invites an
  edit that is silently ignored.
- **Make the placeholder model its own shape.** `YOUR_PREFIX_` keeps the trailing
  separator; `YOUR_TOOL_PREFIX` teaches users to write `personal`, which yields
  `personalsearch_emails`.

## Placeholders are unvalidated, and paths are not shell-expanded

Nothing in the codebase checks that a `YOUR_*` placeholder was replaced — the
literal string flows into the native config file. Documentation carries that
weight entirely.

More sharply: env values are copied **verbatim** into agent config, and MCP
servers are spawned **without a shell**, so `~` is never expanded. A path
placeholder must therefore be absolute. A `~/.gmail-mcp/credentials.json` typed
into a settings field resolves against the agent's working directory — a task
worktree — producing a literal `~` directory inside the user's repository
containing a refresh token, positioned for an agent to commit it.

Note the trap for documentation: the same string in a shell snippet *does*
expand, so a page can show `~` in a terminal command and an absolute path in a
config table and be correct in both places — but it must say why, or the two look
like a typo.

## Prefer a path over a token where the tool allows it

VK is an MCP config **writer**, not a client, so anything in an entry's `env`
lands in plaintext in every assigned agent's global config file. The encrypted
shared gateway is streamable-HTTP only and unavailable to stdio servers.

Gmail's entry carries a credentials *path*, not a token: the refresh token stays
in the server's own file and never enters `~/.claude.json` or
`~/.codex/config.toml`. Where a tool supports it, this is strictly better than
the token-in-env shape — worth stating in a comment so a later change does not
"helpfully" inline the secret.

## No per-user rows in a shipped catalog

The motivating request named three specific mailboxes (personal, and two
employers). Those are **not** three catalog entries. `default_mcp.json` ships to
every user of an open-source product, so a row naming one person's employer or
client publishes private affiliation and is noise for everyone else — and it
leaves the single-instance limitation in place for every other template. Ship one
entry; let users name their instances.

## Ordering gotcha: `meta` must stay last

`preserve_order` is enabled workspace-wide, which makes `Map::remove` in
`extract_meta` a **swap**-remove — it moves the map's last entry into the vacated
slot. This is a no-op only because `meta` is the last key in
`default_mcp.json`. A new catalog entry appended *after* `meta` would silently
take `meta`'s position in every generated agent config. Keep new entries above
it.

## Testing

- The allocator: free key, first collision, second collision, gap-filling
  (`['gmail','gmail_3']` → `gmail_2`, so it is membership-driven not
  counter-driven), unrelated names ignored, and a conflicting name treated as
  taken.
- A **property** test that every generated name matches `^[a-zA-Z0-9_-]+$` —
  this is the assertion that binds the frontend generator to the backend
  validator, and it is the one that would catch a future separator change.
- What tests cannot reach: the tile no longer being `disabled` has no automated
  coverage (no component-test harness exists for that surface), and cross-client
  tool-name collision happens inside the *agent's* MCP client, which VK does not
  host. Both are manual-only; say so rather than implying coverage.
