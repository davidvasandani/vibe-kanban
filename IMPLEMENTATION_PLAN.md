# Implementation Plan: Gmail MCP connector (`vk/4daf-gmail-mcp`)

Companion to [`SPEC.md`](SPEC.md) and [`PRIOR_KNOWLEDGE.md`](PRIOR_KNOWLEDGE.md).

Two independent slices that can land in either order. Slice A (catalog entry) is
data plus tests. Slice B (multi-instance) is frontend logic plus tests. Slice A
is *useful* without B (one Gmail account works today); B is what makes the
three-mailbox request achievable, and B benefits every other template.

## Step 0 — Environment

The toolchain is present but not all of it is on `PATH` in a fresh worktree.

```bash
export PATH="$HOME/.cargo/bin:$PATH"     # cargo/rustc/clippy/rustfmt live here
corepack enable pnpm                      # pnpm 10.13.1 via corepack
pnpm install --frozen-lockfile            # required before any check; no node_modules ships
```

Confirm `cargo --version` and `pnpm --version` both answer before starting. Skip
this and every verification command in Step 7 fails for reasons unrelated to the
change.

---

## Slice A — Gmail catalog entry

### A1. Add the `gmail` server entry

`crates/executors/default_mcp.json`, as a sibling of `slack`:

```json
"gmail": {
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

Constraints that are easy to get wrong:

- The commit-ish must be the **full 40-character SHA**, never `main`. The A3 test
  enforces this, but write it correctly the first time.
- `GMAIL_OAUTH_PATH` is deliberately **absent** — the OAuth client is shared
  across accounts and its default (`~/.gmail-mcp/gcp-oauth.keys.json`) is right.
  Adding it as a placeholder would imply per-account OAuth clients, which is
  wrong.
- `--tool-prefix` goes in `args`, not `GMAIL_MCP_TOOL_PREFIX` in `env`, because
  the flag wins over the env var and two sources invite a silently-ignored edit.

### A2. Add the `meta.gmail` block

In the same file's `meta` object:

```json
"gmail": {
  "name": "Gmail",
  "description": "Read, search, draft and send Gmail from your agent",
  "url": "https://github.com/davidvasandani/Gmail-MCP-Server"
}
```

No `icon` key. The tile falls back to the first initial
(`McpSettingsSection.tsx:1130-1134`), exactly as Slack does. Icons live in
`packages/public/mcp/` and resolve as `/<icon>` — noted here only so a later
change puts one in the right place.

`url` must name the same `owner/repo` as the install spec; A3 asserts it.

### A3. Rust tests in `crates/executors/src/mcp_config.rs`

Add to the existing `mod tests`, next to the Slack tests, plus a module constant
`GMAIL_MCP_FORK_REVISION` holding the pinned SHA so the pin has one named home.

1. `gmail_preconfigured_server_matches_the_documented_stdio_contract`
   Assert `command == "npx"`, the exact `args` array, the exact `env` map, and
   `meta.gmail.name` / `meta.gmail.url`. Mirror
   `slack_preconfigured_server_matches_the_documented_stdio_contract` (line 635).

2. `gmail_preconfigured_server_pins_an_immutable_fork_revision`
   A **shape** test. Add a helper `parse_github_git_spec(spec) -> Option<(owner,
   repo, commit_ish)>` beside the existing `parse_github_release_asset` (line
   654). Then assert:
   - the spec parses as a `github:` git spec;
   - `owner/repo` equals the owner/repo parsed out of `meta.gmail.url`;
   - `commit_ish == GMAIL_MCP_FORK_REVISION` and is 40 lowercase hex characters;
   - the spec contains none of `#main`, `#master`, `refs/heads/`, `@latest`, and
     is not a bare `github:owner/repo` with no fragment.

   Write the rejects as a loop over a `&[&str]` of forbidden substrings with the
   offending value in the failure message, matching how the Slack test reads.
   This is the test that fails for the *next* person reaching for a mutable pin.

3. `gmail_preconfigured_server_adapts_for_codex_and_opencode`
   Model on line 723. Assert Codex retains `command`/`args`/`env`, and Opencode
   produces `type: "local"`, a `command` **array** `["npx", …]`, and
   `environment` (not `env`) carrying `GMAIL_CREDENTIALS_PATH`. This is the test
   that catches the field-rename trap the knowledge base flags.

Deliberately **not** added: a SHA-256 digest constant and a
`pinned-artifacts.yml` audit job. A git commit SHA is resolved and verified by
npm at install time on every machine; the Slack audit exists only because a
release asset is mutable under a fixed tag. Adding an audit here would be
ceremony, and an unaudited constant would be worse than none. `SPEC.md` records
the reasoning; do not "restore parity" with Slack.

Run: `cargo test -p executors gmail`.

---

## Slice B — Multi-instance template instantiation

### B1. `nextAvailableServerName`

`packages/web-core/src/shared/lib/sharedMcpSettingsState.ts`, exported pure
function beside `preconfiguredMcpServers`:

```ts
export function nextAvailableServerName(
  key: string,
  existing: readonly string[]
): string {
  const taken = new Set(existing);
  if (!taken.has(key)) return key;
  let suffix = 2;
  while (taken.has(`${key}_${suffix}`)) suffix += 1;
  return `${key}_${suffix}`;
}
```

The `_` separator is load-bearing: the backend validates names against
`^[a-zA-Z0-9_-]+$` (`shared_mcp_config.rs:208`) and rejects duplicates outright
(`:928`). A space or parenthesis would be silently rewritten by
`suggested_server_identifier` or rejected on save.

### B2. Wire it into `addPreconfigured`

`packages/web-core/src/shared/dialogs/settings/settings/McpSettingsSection.tsx`,
lines 609-627. Replace `setServer({ name: key, … })` with

```ts
const name = nextAvailableServerName(
  key,
  draft.servers.map((server) => server.name)
);
setServer({ name, definition, assignments });
```

Two details:

- Read from `draft.servers` — the same source the tile's `added` flag uses — so
  an unsaved instance still reserves its name. `setServer` de-duplicates by name
  (lines 536-548), so passing a colliding name silently *overwrites* the first
  instance rather than erroring. That is the failure this step prevents.
- `addPreconfigured` is a `useCallback`; add `draft.servers` to its dependency
  array or it will close over a stale list and hand out the same name twice.
  Prefer the functional form if the surrounding code allows it.

### B3. Stop disabling the tile

Same file, lines 1104-1121. Keep computing `added` (it still drives the check
mark and the dimmed styling as an "already added" affordance) but drop
`disabled={added}` and drop the `cursor-default` branch so the tile stays
clickable and keeps its hover affordance.

Re-read the `className` `cn(...)` call afterwards: the `added` branch currently
encodes *both* "looks added" and "is inert". Only the first should survive.

### B4. Frontend tests

`packages/web-core/src/shared/lib/sharedMcpSettingsState.test.ts`:

- `nextAvailableServerName('gmail', [])` → `gmail`
- `nextAvailableServerName('gmail', ['gmail'])` → `gmail_2`
- `nextAvailableServerName('gmail', ['gmail', 'gmail_2'])` → `gmail_3`
- gap case: `['gmail', 'gmail_3']` → `gmail_2` (deterministic, documents that it
  fills gaps rather than counting)
- unrelated names are ignored: `['slack', 'context7']` → `gmail`
- property: every result over a range of taken-sets matches
  `/^[a-zA-Z0-9_-]+$/` — the assertion that binds this generator to the backend
  validator

Run: `pnpm --filter @vibe/web-core test` — the package is named `@vibe/web-core`
(not `@vibe-kanban/web-core`), and its `test` script is `vitest run`.

---

## Step 6 — Documentation

### 6a. `docs/integrations/mcp-server-configuration.mdx`

New **Gmail connector** section after the Slack one (which ends around line 108),
following the repo's Mintlify rules — British spelling, second person, `<Steps>`
for the procedure, `<Warning>` for the prefix trap.

Cover, in this order:

1. Prerequisites: a Google Cloud OAuth client (`gcp-oauth.keys.json` at
   `~/.gmail-mcp/`), plus `git` and network access — the entry installs from a
   git revision and builds on first launch, so the first run is slow.
2. The catalog entry as shipped, with both placeholders called out.
3. One-off consent **per account**, run in a terminal, without `--tool-prefix`:

   ```bash
   GMAIL_CREDENTIALS_PATH=~/.gmail-mcp/credentials-personal.json \
     npx -y github:davidvasandani/Gmail-MCP-Server#<sha> auth
   ```

4. `<Warning>`: each instance needs its own `--tool-prefix`. Without it, some
   clients dedupe tools by base name and one instance silently shadows the
   other — the user sees a working `search_emails` reading the wrong mailbox.
5. The worked three-account table using **neutral** names (`gmail_personal`,
   `gmail_work`, `gmail_client`) — not the requester's employer or client names.
   Columns: server name, `GMAIL_CREDENTIALS_PATH`, `--tool-prefix`.
6. A note that 26 tools per instance adds up, cross-referencing the existing
   "Limit MCP Servers" tip, and that `--scopes` narrows permissions at auth time.

Also add a short line to the **Popular MCP Servers** intro noting that a template
can now be added more than once, since that is a behaviour change users will
notice.

### 6b. `AGENTS.md`

New bullet in **Dependencies**, after the Slack one:

> The bundled **Gmail** MCP catalog entry (`crates/executors/default_mcp.json`)
> pins a **commit SHA** on the `davidvasandani/Gmail-MCP-Server` fork. Renovate
> cannot track a bare SHA on a release-less fork, so this pin is **bumped by
> hand**: move the SHA in `default_mcp.json`, the `GMAIL_MCP_FORK_REVISION`
> constant in `crates/executors/src/mcp_config.rs`, and the revision named in
> `docs/integrations/mcp-server-configuration.mdx` **together**. Unlike Slack,
> there is no digest constant and no audit job — a git SHA is immutable, so npm
> verifies it at install time.

`CLAUDE.md` is a symlink to `AGENTS.md`; edit `AGENTS.md` only.

### 6c. Renovate

No manager is added. If a comment is placed in `renovate.json`, it must say the
Gmail pin is intentionally unmanaged — not add a manager that would match the pin
and never propose anything.

---

## Step 7 — Verification

In order; each gate is cheap relative to the next.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test -p executors gmail          # A3
cargo test -p executors                 # no Slack/adapter regressions
pnpm --filter @vibe/web-core test       # B4
pnpm run check                          # frontend + all Rust workspaces
pnpm run lint
pnpm run format                         # required before completing the task
```

`pnpm run generate-types` is **not** needed — no Rust type changed. If
`generate-types:check` is run and reports a diff, something unintended happened;
investigate rather than committing the regenerated file.

### Manual verification (the part tests cannot reach)

Cache isolation is the whole point — a warm cache proves the *previous* artifact
works.

1. ~~**The install builds.**~~ **Done** — with an isolated `npm_config_cache`,
   `npm install github:davidvasandani/Gmail-MCP-Server#030da34…` succeeded in
   53 s and produced an executable `dist/index.js` (78 KB). `prepare` runs.
2. ~~**The server speaks MCP with a prefix.**~~ **Done** — over stdio,
   `initialize` returned `serverInfo {"name":"gmail","version":"1.0.0"}` and
   `tools/list` returned **28** tools (the README's list of 26 omits `get_thread`
   and `list_inbox_threads`), every one prefixed by `--tool-prefix=personal_`.

   Two findings from this run that the implementation must respect:
   - The server **exits before `initialize`** when `gcp-oauth.keys.json` is
     absent, so VK's "Test connection" will report `failed` for a user who has
     not yet created a Google Cloud OAuth client. The docs must lead with that
     prerequisite.
   - The keys file only needs to *exist* to serve `tools/list` — it need not be
     valid, and `credentials.json` is not read until a tool is called. So a user
     who does the OAuth-client step but skips per-account consent gets a
     connector that tests green and fails at first use. Give the consent step
     equal weight in the docs.
3. **Two instances coexist in the UI.** Settings → MCP Servers → add Gmail twice;
   confirm `gmail` and `gmail_2` appear as separate cards, rename both, and give
   each its own credentials path and prefix. Save and confirm both land in the
   assigned agent's native config as distinct keys.
4. **Two mailboxes, if credentials are available.** Authorise two accounts and
   confirm each prefixed tool set reads its own mailbox.

Steps 3-4 need a running app and real Google credentials. If they cannot be
completed in this environment, say so explicitly in the task summary rather than
implying they passed.

---

## Step 8 — Knowledge distillation (pipeline stage 12)

Material this task produces that is genuinely reusable:

- Running multiple instances of one catalog template: instance-name allocation,
  the `^[a-zA-Z0-9_-]+$` binding between the frontend generator and the backend
  validator, and why per-account credentials *require* per-account names.
- Client-side tool-name dedupe across MCP servers as a failure mode, and
  `--tool-prefix` as the remedy.
- Pinning a **git commit SHA** instead of a release asset, and which integrity
  layer that removes the need for — the reasoning that justifies diverging from
  the Slack idiom in `forked-mcp-server-packaging.md`. That page's
  rejected-alternatives row for `npx github:owner/repo#<sha>` should be amended
  to note its objection is Go-specific.
- Correct the stale claim in `shared-mcp-configuration.md` that the settings UI
  does not render catalog suggestions.

Add or update pages in the knowledge base the pipeline targets, tag with
`vk/4daf-gmail-mcp`, and refresh the index.

---

## Risks to watch while implementing

| Risk | Signal | Response |
| --- | --- | --- |
| `npx github:…#sha` does not build (no `prepare`, `ignore-scripts`, missing git) | Step 7 manual check 1 fails | Fall back to `@artymclabin/gmail-mcp@1.2.3` per `SPEC.md`'s rejected-alternative, and revise the spec rather than shipping a broken entry |
| Stale closure in `addPreconfigured` | Two adds produce one server | Dependency array or functional update (B2) |
| `setServer` overwrite masks a naming bug | Second add silently replaces the first | The B4 "two distinct draft servers" test |
| Codex `meta` pruning drops `gmail_2` | Cosmetic only | Documented in `SPEC.md`; do not "fix" it here |
| Overreach into a display-label feature | Diff grows a new field on `SharedMcpServer` | Out of scope — there is nowhere to persist it |
