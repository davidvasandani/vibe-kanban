# Technical Spec: Gmail MCP connector with multi-account instances

Task id: `vk/4daf-gmail-mcp`

> Constraints distilled from the project knowledge base are in
> [`PRIOR_KNOWLEDGE.md`](PRIOR_KNOWLEDGE.md); the load-bearing ones are folded
> into the design sections below and cited where they apply.

## Summary

Add a bundled **Gmail** MCP connector to Vibe Kanban's popular-servers catalog,
installed from the requester's fork
([`davidvasandani/Gmail-MCP-Server`](https://github.com/davidvasandani/Gmail-MCP-Server)),
and make the catalog able to produce **several independent instances of the same
template** so one user can run Gmail against multiple Google accounts
side-by-side (personal plus two work mailboxes, in the motivating case).

Two changes, deliberately separated:

1. **Catalog entry** (`gmail`) — a new preconfigured stdio server pinned to an
   immutable fork revision, with placeholder values for the per-account OAuth
   token store and the tool-name prefix.
2. **Multi-instance template instantiation** — today a template tile is keyed by
   its catalog key and disables itself once a logical server of that exact name
   exists, so a second Gmail cannot be created. Template instantiation is changed
   to allocate a fresh, valid identifier instead of a fixed one.

Change 2 is generic. It is not Gmail-specific, and every existing template
benefits (two Slack workspaces, two Context7 keys, and so on).

## Motivation

The Gmail MCP server exposes 28 tools for reading, searching, drafting, sending,
labelling and filtering mail. It is the kind of connector a user wants pointed at
more than one mailbox at once, because the agent's job frequently spans them
("find the thread where X was agreed, and draft the reply from my work address").

Google's OAuth model makes one server instance strictly one account: the server
holds a single refresh token loaded from a single credentials file. Multiple
accounts therefore mean multiple *processes*, each with its own credentials path.
That is a client-configuration problem, and Vibe Kanban's MCP settings screen is
where it has to be solved.

### Non-goals

- Vibe Kanban does **not** perform the Google OAuth consent flow. The server owns
  it (`auth` subcommand, loopback listener, browser consent). We document it; we
  do not wrap it. See [Rejected alternatives](#rejected-alternatives).
- No Gmail-specific UI, no mailbox picker, no credential storage in Vibe Kanban.
  Credentials stay in the files the Gmail server already owns.
- No change to how logical servers are written into agent-native config files,
  tested, assigned to executors, or reconciled.
- Not shipping the requester's specific account labels ("Sweetgreen",
  "Proalign") anywhere in the repository. See
  [Naming](#naming-instances-is-the-users-job-not-the-catalogs).

## Background: what the upstream server actually supports

Facts established against the fork at revision
`030da3492753222a41645a9f343466d151c63f3c` (fork `main`, the only branch) and
npm `@artymclabin/gmail-mcp@1.2.3`.

| Aspect | Behaviour |
| --- | --- |
| Transport | stdio only (`StdioServerTransport`) |
| Entry point | `bin.gmail-mcp` → `dist/index.js`; `"prepare": "npm run build"` |
| OAuth client keys | `GMAIL_OAUTH_PATH`, default `~/.gmail-mcp/gcp-oauth.keys.json` |
| Account token store | `GMAIL_CREDENTIALS_PATH`, default `~/.gmail-mcp/credentials.json` |
| Tool-name prefix | `GMAIL_MCP_TOOL_PREFIX` env, or `--tool-prefix=<v>` flag (flag wins) |
| Scope narrowing | `--scopes=gmail.readonly,…` at auth time |
| Consent flow | `node dist/index.js auth`, interactive, browser + `http://localhost:3000/oauth2callback` |

The fork is currently byte-identical to upstream `ArtyMcLabin/Gmail-MCP-Server`
(`ahead_by: 0`, `behind_by: 0`) and publishes no GitHub releases.

### Measured, not assumed

The pinned revision was installed and driven over stdio before this spec was
finalised, with an isolated `npm_config_cache`:

- `npm install github:davidvasandani/Gmail-MCP-Server#030da34…` succeeds in ~53 s
  cold and produces an executable `dist/index.js`, confirming `prepare` runs.
- `initialize` returns `serverInfo: {"name":"gmail","version":"1.0.0"}`.
- `tools/list` returns **28** tools, not the 26 the README enumerates (it omits
  `get_thread` and `list_inbox_threads`). Use 28 in docs and capacity reasoning.
- With `--tool-prefix=personal_`, every returned tool name carries the prefix.

**The server exits before `initialize` if `gcp-oauth.keys.json` is missing.** It
writes `OAuth keys file not found. Please place gcp-oauth.keys.json in current
directory or ~/.gmail-mcp` to stderr and terminates — the MCP handshake never
completes. This is a hard prerequisite, not a soft degradation, and it has a
direct product consequence: **Vibe Kanban's "Test connection" reports `failed`
for a user who adds the Gmail template before creating a Google Cloud OAuth
client.** The docs must lead with the prerequisite so that result reads as
"unfinished setup" rather than "broken connector".

Notably the keys file only has to *exist* to serve `tools/list`; it does not have
to be valid, and `credentials.json` is not needed until a tool is actually
called. So a user who completes the OAuth-client step but not the per-account
consent step gets a connector that tests green and fails at first use.

Three consequences drive the design.

**The OAuth client is shared; the token store is per account.**
`gcp-oauth.keys.json` identifies the *Google Cloud project*, not the mailbox. All
instances can and should point at one `GMAIL_OAUTH_PATH`. Only
`GMAIL_CREDENTIALS_PATH` must differ per account. The catalog entry therefore
leaves `GMAIL_OAUTH_PATH` at its default (absent from the entry) and makes
`GMAIL_CREDENTIALS_PATH` an explicit placeholder.

**A distinct tool prefix is mandatory, not cosmetic.** The upstream README is
explicit: *"Some MCP clients dedupe tool entries by their base name across
servers, which makes it impossible to run two instances of this server
side-by-side."* Two unprefixed Gmail instances do not degrade gracefully — one
silently shadows the other, and the user sees a working `search_emails` that
reads the wrong mailbox. The prefix is in the catalog entry as a required
placeholder for exactly this reason.

**`auth` must not carry the prefix.** The README notes the `auth` subcommand runs
independently and should be invoked without `--tool-prefix`. The documented
consent procedure therefore sets `GMAIL_CREDENTIALS_PATH` for the account being
authorised and nothing else.

## Design

### Install spec: pin the fork by commit SHA

```json
{
  "command": "npx",
  "args": [
    "-y",
    "github:davidvasandani/Gmail-MCP-Server#030da3492753222a41645a9f343466d151c63f3c",
    "--tool-prefix=YOUR_TOOL_PREFIX"
  ],
  "env": {
    "GMAIL_CREDENTIALS_PATH": "YOUR_CREDENTIALS_PATH"
  }
}
```

The pin is a **full 40-character commit SHA, never `main`**. A branch reference
would let the installed code change under an unchanged repository — both a
supply-chain hazard and an unreproducible-build hazard — and is exactly what the
catalog's shape test exists to reject.

Because `package.json` declares `"prepare": "npm run build"`, npm compiles the
TypeScript when installing from a git spec, so this produces a working
`dist/index.js` without the fork needing to cut a release.

**Why this contradicts the Slack precedent, and why that is correct.**
`docs/knowledge-base/forked-mcp-server-packaging.md` lists
`npx github:owner/repo#<sha>` in its rejected-alternatives table:
*"Pinned, but clones the whole (Go) repo per cache miss and still needs a binary
source at run time."* Both halves of that objection are properties of the **Slack
fork specifically**, which is a Go program: a git checkout of Go source is not
runnable by `npx`, so a launcher that downloads a compiled binary was
unavoidable. Gmail's fork is a TypeScript npm package with a `prepare` script, so
the checkout *is* the runnable artifact and there is no second binary source.
The rejection does not transfer, and this spec records that explicitly so the
divergence reads as a decision rather than an oversight.

**Why no digest constant and no audit workflow.** Slack needs
`SLACK_MCP_LAUNCHER_SHA256` and the daily
`.github/workflows/pinned-artifacts.yml` job because a GitHub *release asset* can
be replaced under an existing tag — the pin names a mutable location. A git
commit SHA names an immutable object: npm resolves it to exactly one tree, so the
SHA **is** the integrity value and it is enforced at install time on every user's
machine, not audited after the fact. Adding a Gmail audit job would be ceremony
against a threat that the pin already closes. That is a strictly stronger
position than Slack's, not a weaker one.

Cost accepted: a git install builds from source, so first launch on a cold npm
cache is slower than a published tarball and requires `git` plus the ability to
install dev dependencies. Subsequent launches hit the npx cache. This is stated
in the docs as a prerequisite.

Renovate cannot track a bare commit SHA on a fork with no releases. Rather than
add a manager that appears to give coverage and does not — the
`ignoreUnstable: false` lesson from the Slack integration, where a manager
matched the pin and then never proposed anything — the Gmail pin is documented as
manually bumped in `AGENTS.md`. A known-manual pin is safer than fictitious
automation.

### `--tool-prefix` on argv, credentials in env

The prefix goes in `args` rather than `GMAIL_MCP_TOOL_PREFIX` because the flag
takes precedence over the env var; keeping one authoritative source avoids a
configuration where a user edits the env var and is silently ignored. The
credentials path has no flag form, so it stays in `env`.

Both placeholders follow the catalog's existing `YOUR_*` convention
(`YOUR_API_KEY`, `YOUR_TOKEN`), which is the "edit this before it works" signal
users already recognise. Note that nothing in the codebase validates that a
placeholder was replaced — they flow untouched into the native config file — so
the docs carry that weight.

### A path, not a secret

Every other credential-bearing catalog entry puts a live secret in `env`
(`SLACK_MCP_XOXP_TOKEN`, `EXA_API_KEY`), and those land in plaintext in each
assigned agent's global config file. The shared MCP gateway that encrypts
upstream tokens in SQLite is only available to OAuth-capable **streamable HTTP**
servers; a stdio server cannot use it.

Gmail's entry carries a *filesystem path*, not a token. The refresh token stays
in `~/.gmail-mcp/credentials-*.json` under the Gmail server's own ownership and
never enters `~/.claude.json`, `~/.codex/config.toml`, or any other agent config.
This is a genuine improvement over the Slack shape and is worth stating so a
future change does not "helpfully" inline the token.

### Multi-instance template instantiation

Current behaviour in `packages/web-core/src/shared/dialogs/settings/settings/McpSettingsSection.tsx`:

- `addPreconfigured(key, entry)` calls `setServer({ name: key, … })` — the new
  logical server always takes the catalog key verbatim (line 624).
- `setServer` filters out any existing draft server of the same name before
  appending (lines 536–548), so a repeat add would overwrite rather than add.
- The tile computes `added = draft.servers.some(s => s.name === server.key)` and
  sets `disabled={added}` (lines 1104–1115).

So the first Gmail becomes `gmail`, the tile greys out permanently, and a second
instance is unreachable. (There is an accidental workaround today: rename the
first instance and the tile re-enables. That is a side effect of name-based
matching, not a designed path, and it leaves the user to invent identifiers with
no guidance.)

**Change:** `addPreconfigured` allocates the first free identifier in the
sequence `key`, `key_2`, `key_3`, … against the names already in the draft, and
the tile is no longer disabled. The check mark stays as an "already added"
affordance but stops blocking.

The suffix is `_2` — underscore, not a space or parenthesis — because the backend
validates logical server names against `^[a-zA-Z0-9_-]+$` in
`is_valid_server_identifier` (`crates/executors/src/shared_mcp_config.rs:208`).
These names are protocol identifiers written into agent-native config files, not
display labels; generating a name the backend would reject, and that
`suggested_server_identifier` would then rewrite behind the user's back, is the
failure mode that rule exists to prevent. The backend also rejects duplicates
outright (`validate_write_request`, `shared_mcp_config.rs:928`), so allocation
must be correct rather than merely convenient.

Allocation reads the **draft** server list — the same source the tile's `added`
flag already uses — so a name allocated for an unsaved server is not handed out
twice within one editing session.

### Naming: instances are the user's job, not the catalog's

The request names three instances: Gmail MCP (Personal), Gmail MCP (Sweetgreen),
Gmail MCP (Proalign). Those are **not** encoded as three catalog entries.

`default_mcp.json` ships to every user of an open-source product. Baking one
user's employer and client names into it would publish private affiliation, be
meaningless to everyone else, and set a precedent that the catalog grows per-user
rows. The catalog ships one `gmail` entry; the user renames each instance in the
server editor, which the UI already supports (`McpSettingsSection.tsx:568-600`
handles rename, removing the old key and re-adding under the new one).

There is also no display-label concept to hang "Gmail MCP (Personal)" on:
`SharedMcpServer` has only `name` (`shared_mcp_config.rs:87`), and native agent
configs store nothing but the map key, so a label would have nowhere to persist
without introducing the first Vibe-Kanban-owned MCP store. Out of scope.

Concretely the user ends up with `gmail_personal`, `gmail_sweetgreen`,
`gmail_proalign` (or any identifiers they choose), each with its own
`GMAIL_CREDENTIALS_PATH` and its own `--tool-prefix`. The docs show exactly this
shape using neutral example names.

### Assets and metadata

The `meta` block gains a `gmail` object — `name: "Gmail"`, a one-line
description, and `url` pointing at the fork. `meta.<server>.url` is a link shown
in the UI and has no effect on what is installed, so the catalog contract
requires asserting that it names the same repository the install spec builds
from; the shape test below does that.

No icon ships. Catalog icons live in `packages/public/mcp/` and resolve as
`/<icon>`; the tile already falls back to the server's first initial when `icon`
is absent (`McpSettingsSection.tsx:1130-1134`), which is what the Slack entry
does today. Gmail's logo is a Google trademark whose redistribution terms are not
worth assuming for a cosmetic gain.

### Adaptation across executors

The entry is plain stdio, so it flows through the existing per-executor adapters
unchanged, with one that matters: Opencode renames the stdio `env` field to
`environment` (`mcp_config.rs:480`). Dropping or mis-shaping that makes
credential-dependent entries unusable after adaptation, so the Gmail entry gets
the same Codex/Opencode adaptation test the Slack entry has.

One known cosmetic wrinkle: `adapt_codex` prunes `meta` to keys present in
`servers` (`mcp_config.rs:418-423`), so instances named `gmail_2` have no
corresponding `meta` entry. `meta` is presentation-only and Codex's handling of
it is already unusual (it inserts `meta` into the servers map). Not fixed here;
recorded so it is not mistaken for a regression.

## Interfaces

No new API endpoints, no new Rust types, no ts-rs regeneration. The catalog is
data reaching the frontend through the existing `preconfigured` field on
`SharedMcpReadResponse` (`shared_mcp_config.rs:432`, which returns the canonical
unadapted catalog), and `preconfiguredMcpServers()` already derives tiles
generically from `meta` (`sharedMcpSettingsState.ts:33-66`).

The one behavioural interface change is internal to the settings screen:

```ts
// packages/web-core/src/shared/lib/sharedMcpSettingsState.ts
export function nextAvailableServerName(
  key: string,
  existing: readonly string[]
): string;
```

Pure, exported for direct unit test, consumed by `addPreconfigured`.

## Testing

Rust — `crates/executors/src/mcp_config.rs`, alongside the Slack tests:

- `gmail_preconfigured_server_matches_the_documented_stdio_contract` — exact
  `command`, `args`, `env`, and `meta.gmail.name`/`url`, matching what the docs
  page states.
- `gmail_preconfigured_server_pins_an_immutable_fork_revision` — a **shape** test,
  not a string test: parse the spec into `owner/repo` + commit-ish, assert
  `owner/repo` equals the owner/repo in `meta.gmail.url`, assert the commit-ish
  is 40 hex characters, and reject `#main`, `#master`, `refs/heads/`, `@latest`,
  and a bare repo reference. This fails for the *next* person who reaches for a
  mutable pin, not just for today's.
- `gmail_preconfigured_server_adapts_for_codex_and_opencode` — survives
  per-executor adaptation; Opencode gets `type: "local"`, a `command` array, and
  `environment`.

TypeScript — `packages/web-core/src/shared/lib/sharedMcpSettingsState.test.ts`:

- `nextAvailableServerName` returns `gmail` when free, `gmail_2` when `gmail` is
  taken, `gmail_3` when both are, and is deterministic when the taken set has
  gaps.
- Every generated name satisfies `^[a-zA-Z0-9_-]+$` — the property that binds the
  frontend generator to the backend validator.
- Adding a template twice yields two distinct draft servers rather than one
  overwritten one.

Manual verification, because the parts most likely to break are the ones tests
cannot reach — the git install actually building, and two prefixed instances
coexisting. Cache isolation is the point: a warm cache proves nothing.

1. With a fresh `npm_config_cache`, run
   `npx -y github:davidvasandani/Gmail-MCP-Server#<sha>` and drive it over stdio
   (`initialize` → `notifications/initialized` → `tools/list`); confirm the tools
   list returns and carries the configured prefix.
2. Add Gmail twice in Settings → MCP Servers; confirm `gmail` and `gmail_2`, then
   rename both and give each a distinct credentials path and prefix.
3. Authorise two accounts, running `auth` once per account with
   `GMAIL_CREDENTIALS_PATH` set and no `--tool-prefix`.
4. Assign both to one agent and confirm both prefixed tool sets appear and that
   each reads its own mailbox.

## Documentation

`docs/integrations/mcp-server-configuration.mdx` gains a **Gmail connector**
section after the Slack one: the Google Cloud OAuth client prerequisite, the
one-off interactive `auth` per account with `GMAIL_CREDENTIALS_PATH` set, why
each instance needs its own `--tool-prefix` and what happens if it does not, the
worked three-account example with neutral names, and the cold-start build note.

`docs/knowledge-base/shared-mcp-configuration.md` and
`forked-mcp-server-packaging.md` are updated in the knowledge-distillation stage,
not here — including the stale claim in the former that *"the current shared MCP
settings UI does not render catalog suggestions"*, which the settings screen has
since contradicted.

`AGENTS.md` gains a Gmail bullet under **Dependencies** stating that the pin is a
fork commit SHA outside Renovate's reach and must be bumped by hand together with
the docs page — the same cross-file coupling note the Slack entry carries.

## Risks

| Risk | Mitigation |
| --- | --- |
| Git-install build fails on a user's machine (no `git`, offline, `ignore-scripts=true` suppressing `prepare`) | Documented prerequisite; failure is a loud npx error at agent launch, not a silent degradation |
| User adds the template before creating a Google Cloud OAuth client; "Test connection" reports `failed` and looks like a broken connector | Docs lead with the prerequisite; the server's own stderr names the missing file, and the diagnostic surfaces in the test result |
| Keys file present but per-account consent never run — connector tests green, fails at first tool call | Documented as a two-step setup with the consent step given equal weight |
| Fork pin goes stale because Renovate cannot see it | Recorded explicitly in `AGENTS.md`; no fake automation |
| User forgets a distinct `--tool-prefix` and one instance silently shadows the other | Placeholder is `YOUR_TOOL_PREFIX` so an unedited entry is visibly wrong; docs state the consequence |
| User points two instances at one `GMAIL_CREDENTIALS_PATH` | Both read the same mailbox — confusing, not destructive; docs call it out |
| 28 tools × 3 instances = 84 tools degrades agent performance | Existing "Limit MCP Servers" guidance applies; `--scopes` narrowing documented as the lever |
| Auto-suffixed `gmail_2` is opaque next to a renamed `gmail_personal` | The server editor opens immediately after adding, with the name field editable |

## Rejected alternatives

**Three hard-coded catalog entries (`gmail_personal`, `gmail_sweetgreen`,
`gmail_proalign`).** Literally what was asked for, and the smaller diff. Rejected
because it publishes one user's employer and client names in an open-source
repository, ships two rows that are noise for every other user, and leaves the
single-instance limitation in place for every other template.

**Install from npm `@artymclabin/gmail-mcp@1.2.3`.** Faster cold start, no build
step, `dist.integrity` verified by npm, and Renovate-trackable — the packaging
knowledge base names an exact registry version as the *preferred* shape.
Rejected because the requester explicitly asked for their fork; a fork with zero
divergence today is a fork intended to diverge tomorrow, and switching install
sources later is a user-visible reconfiguration. This remains the fallback if
cold-start build time proves painful in practice.

**Ask the fork to cut a release tarball, Slack-style.** The most robust option if
the fork later gains a build pipeline. Rejected for now because it requires work
in a repository outside this task's scope, and the `prepare` script makes it
unnecessary for correctness — the git SHA already provides install-time integrity
that the Slack release-asset shape cannot.

**Vibe Kanban drives the Google OAuth consent flow in-app.** Would remove the
terminal step. Rejected: it means owning Google client secrets, a redirect
listener, and token refresh for a third-party server that already implements all
three — and the shared-gateway OAuth path targets streamable HTTP servers,
whereas this server is stdio-only.

**Free-text name prompt when adding a template.** More explicit than
auto-suffixing, but adds a modal to a flow that currently takes one click, and
the server editor opens immediately afterwards with the name field editable
anyway.
