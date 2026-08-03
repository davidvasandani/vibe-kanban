# Implementation Plan: Gmail MCP connector with multi-account instances

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context

- **Rust** (workspace, edition per `rust-toolchain.toml`) — `crates/executors`
  owns the bundled MCP catalog and its per-agent adaptation.
- **TypeScript / React** — `packages/web-core` owns the shared MCP settings
  surface used by both `local-web` and `remote-web`.
- **No new dependencies.** No new API routes, no new Rust types, no ts-rs
  regeneration. The catalog is data; the frontend change is one pure function
  plus two call sites.
- **Toolchain, fresh worktree**: `cargo` is at `~/.cargo/bin` and not on `PATH`;
  `pnpm` comes via `corepack`; `node_modules` is absent until
  `pnpm install --frozen-lockfile`.

Constraints that shape the design:

- Catalog entries are compiled in (`include_str!`), so a change needs a rebuild
  and cannot be hot-configured.
- Logical MCP server names are **protocol identifiers**, validated against
  `^[a-zA-Z0-9_-]+$` and required to be unique
  (`crates/executors/src/shared_mcp_config.rs:208`, `:928`).
- Vibe Kanban writes agent-native config files; it is not an MCP client at
  runtime. Anything in an entry's `env` lands in plaintext in each assigned
  agent's global config.
- The encrypted shared MCP gateway is streamable-HTTP only, so a stdio server
  cannot use it.

## Architecture & Approach

Two independent slices. Slice A is data plus tests; Slice B is frontend logic
plus tests. Neither depends on the other; A alone gives one Gmail account, B is
what makes several possible.

### Slice A — the catalog entry (FR-1 … FR-5, FR-14, FR-15)

**`crates/executors/default_mcp.json`** gains a `gmail` server entry and a
`meta.gmail` block, as a sibling of `slack` (`default_mcp.json:47-58`, meta at
`:59-101`).

```json
"gmail": {
  "command": "npx",
  "args": [
    "-y",
    "github:davidvasandani/Gmail-MCP-Server#030da3492753222a41645a9f343466d151c63f3c",
    "--tool-prefix=YOUR_TOOL_PREFIX"
  ],
  "env": { "GMAIL_CREDENTIALS_PATH": "YOUR_CREDENTIALS_PATH" }
}
```

- **FR-2** — full 40-hex commit SHA, never `main`. Immutable and verified by npm
  at install time.
- **FR-3** — two `YOUR_*` placeholders, matching the catalog's existing
  convention. Nothing in the codebase validates that placeholders were replaced,
  so the docs carry that weight.
- **FR-4** — `GMAIL_OAUTH_PATH` is deliberately **absent**. The OAuth client is
  per Google Cloud project, not per mailbox, so its default
  (`~/.gmail-mcp/gcp-oauth.keys.json`) is correct and shared. Adding it as a
  placeholder would wrongly imply per-account OAuth clients.
- **FR-9** — `--tool-prefix` lives in `args`, not `GMAIL_MCP_TOOL_PREFIX` in
  `env`, because the flag takes precedence over the env var; one authoritative
  source prevents a silently-ignored edit.
- **FR-5** — the entry is plain stdio, so `Adapter::Passthrough` / `Codex` /
  `Opencode` / `Gemini` / `Cursor` / `Copilot` (`mcp_config.rs:360-533`) handle
  it with no new branch. The one that matters is Opencode renaming `env` →
  `environment` (`mcp_config.rs:480`); a test pins it.

**`meta.gmail`** carries `name: "Gmail"`, a one-line description, and `url`
pointing at the fork. No `icon` (C5) — the tile falls back to the first initial
(`McpSettingsSection.tsx:1130-1134`), as Slack does. `meta.<server>.url` is a
link, not a build instruction, so FR-14's test asserts it names the same
`owner/repo` as the install spec.

**Tests** in `crates/executors/src/mcp_config.rs` `mod tests`, beside the Slack
ones, with a module constant `GMAIL_MCP_FORK_REVISION` giving the pin one named
home (FR-15):

| Test | Models on | Asserts |
| --- | --- | --- |
| `gmail_preconfigured_server_matches_the_documented_stdio_contract` | `:635` | exact `command`/`args`/`env`, `meta.gmail.name`/`url` |
| `gmail_preconfigured_server_pins_an_immutable_fork_revision` | `:666` | shape test — see below |
| `gmail_preconfigured_server_adapts_for_codex_and_opencode` | `:723` | Codex keeps `env`; Opencode gets `type:"local"`, `command` array, `environment` |

The provenance test is a **shape** test, not a string test — the knowledge base
is explicit that this is what makes it fail for the *next* person, not just
today's. Add `parse_github_git_spec(spec) -> Option<(owner, repo, commit_ish)>`
beside the existing `parse_github_release_asset` (`:654`), then assert: the spec
parses; `owner/repo` equals the owner/repo parsed from `meta.gmail.url`;
`commit_ish == GMAIL_MCP_FORK_REVISION` and is 40 lowercase hex characters; and
the spec contains none of `#main`, `#master`, `refs/heads/`, `@latest`, and is
not a fragment-less bare repo reference.

**Deliberately absent**: a SHA-256 constant and a `pinned-artifacts.yml` audit
job. See `research.md` R3 and Constitution XVI — a content-addressed pin *is* the
integrity record.

### Slice B — multiple instances (FR-6 … FR-10)

Three blockers exist today in
`packages/web-core/src/shared/dialogs/settings/settings/McpSettingsSection.tsx`:

1. `addPreconfigured` hard-codes `setServer({ name: key, … })` (`:624`).
2. `setServer` filters out any draft server of the same name before appending
   (`:536-548`) — so a repeat add **overwrites** rather than errors.
3. The tile sets `disabled={added}` where `added` is an exact name match
   (`:1104-1115`).

The fix:

**B1 — `nextAvailableServerName`** in
`packages/web-core/src/shared/lib/sharedMcpSettingsState.ts`, exported and pure
so it is directly unit-testable (see `contracts/`). Returns `key` when free, else
the first free `key_2`, `key_3`, … The `_` separator is load-bearing: FR-7
requires the generated name to be accepted on save, and the backend validator is
`^[a-zA-Z0-9_-]+$`. A space or parenthesis would be rejected, or silently
rewritten by `suggested_server_identifier` — Constitution XXII forbids exactly
that.

**B2 — wire it in** at `:609-627`, allocating against `draft.servers.map(s =>
s.name)`. Two hazards: `addPreconfigured` is a `useCallback`, so `draft.servers`
must enter its dependency array (or use the functional update form) or it closes
over a stale list and hands out one name twice; and because `setServer`
de-duplicates, a colliding name silently destroys the first instance rather than
surfacing an error (FR-6).

**B3 — stop disabling the tile** at `:1104-1121`. Keep computing `added` — it
still drives the check mark and dimmed styling as an "already added" affordance —
but drop `disabled={added}` and the `cursor-default` branch. The `added` branch
of the `cn(...)` call currently encodes both "looks added" and "is inert"; only
the first survives.

**FR-8** needs no work: rename already exists (`:568-600`, removing the old key
and re-adding under the new one). **FR-10** is satisfied by the per-instance
`--tool-prefix` from Slice A — verified against the real server, which returned
28 tools all carrying the configured prefix.

### Setup and failure reporting (FR-11 … FR-13)

Documentation only; no code. Measured behaviour drives it: the server **exits
before completing the MCP handshake** when `gcp-oauth.keys.json` is missing,
writing the missing path to stderr. So VK's existing connection test reports
`failed` and carries that text (FR-12) — already correct through the existing
diagnostic path, which Constitution XI requires to be preserved verbatim.

The subtle case FR-11 must cover: the keys file only has to *exist* to serve
`tools/list`; `credentials.json` is not read until a tool is called. A user who
does the OAuth-client step but skips per-mailbox consent gets a connector that
**tests green and fails at first use**, so the two steps get equal weight.

## Data Model

Not applicable — no entities, no persistence. The catalog is a compiled-in JSON
document; logical servers persist only into agent-native config files, which this
change does not restructure.

## Contracts

See [`./contracts/`](./contracts/) — one internal function contract
(`nextAvailableServerName`) and the catalog entry's shape contract. No HTTP API
changes.

## Research Notes

See [`./research.md`](./research.md) — the fork-vs-npm decision, the measured
behaviour of the pinned revision, why the Slack delivery idiom does not transfer,
and why no audit job is added.

## Constitution Check

Checked against v0.20.0.

| Principle | Status |
| --- | --- |
| I — Clarity over cleverness | ✅ One pure function and a data entry; every non-obvious choice (absent `GMAIL_OAUTH_PATH`, `_2` separator, flag-over-env, no audit job) is justified in the spec or `research.md`. |
| II — Test the contract | ✅ Three Rust tests, six TypeScript cases including a property binding the generator to the backend validator; acceptance criteria stated before implementation. |
| III — Small, reversible steps | ✅ Smallest change: no new types, routes, or dependencies. Generalises the existing template flow rather than duplicating it. |
| IV — Shared-component boundaries | ✅ Logic lands in `web-core` (shared container tier); no `packages/ui` change. Blast radius is both frontends — stated, and the change is behaviour-preserving for every other template. |
| VI — Don't rebuild what shipped | ✅ Reuses `preconfiguredMcpServers`, `setServer`, the rename path, the existing adapters and diagnostic surface. |
| X — Dialogs hold provisional state | ✅ Untouched. Allocation happens before `setServer`; the dialog's seed-on-open contract is unchanged. |
| XI — Diagnostics are evidence | ✅ The missing-keys failure surfaces the server's own stderr through the existing test path, unmodified. |
| XVI — Bundled entries install what they advertise | ✅ Commit SHA is immutable and content-addressed; `meta.gmail.url` and the install spec name the same repo, asserted by a shape test; integrity enforced at install time by npm, so per the clarified XVI no companion audit is required. Docs, pin, and test constant move together. |
| XXII — Templates are not singletons | ✅ The reason this feature exists. One entry, N instances, placeholders for every per-instance value (`GMAIL_CREDENTIALS_PATH`, `--tool-prefix`), no per-user rows, generated identifiers valid by construction against `^[a-zA-Z0-9_-]+$`, and the collision-disambiguator shipped rather than left to discovery. |
| Constraints — transport-neutral catalog entries | ✅ `command`/`args`/`env` with placeholders; per-agent shape left to the adapters. |
| Constraints — generated files never hand-edited | ✅ No Rust type changed, so `shared/types.ts` is untouched and `generate-types` is not run. |
| Constraints — `pnpm run format` before completing | ✅ In the verification sequence. |
| XIV — Repository verification is worktree-safe | ⚠️ Not a change this task makes, but the plan's Step 0 documents the `PATH` and `corepack` prerequisites, because a fresh worktree fails every verification command without them. |

No deviations. Two provisions were *clarified* by this task rather than deviated
from: XVI now states that a content-addressed pin needs no companion audit, and
XXII is new. Both were written in stage 4 before this plan, so this plan is
checked against them rather than justifying them retroactively.

## Risks & Dependencies

| Risk | Signal | Response |
| --- | --- | --- |
| Git install fails on a user's host (`ignore-scripts=true` suppressing `prepare`, no `git`, offline) | Loud npx error at agent launch | Documented prerequisite; C4 names this as a reopen condition for the npm fallback |
| Stale closure in `addPreconfigured` | Two adds yield one server | Dependency array / functional update (B2); covered by the "two distinct draft servers" test |
| `setServer` de-dup masks a naming bug destructively | Second add silently replaces the first | Same test; this is why B2 allocates *before* calling `setServer` |
| Codex `meta` pruning drops `gmail_2` | Cosmetic only — `meta` is presentation | Documented in `SPEC.md`; explicitly not fixed here |
| Scope creep into a display-label field | A new field appears on `SharedMcpServer` | Out of scope — agent-native config stores only the map key, so there is nowhere to persist it |
| 28 tools × 3 instances = 84 tools degrades the agent | — | Existing "Limit MCP Servers" guidance; `--scopes` documented as the lever |

**Dependencies**: none added. External dependency is the fork remaining
reachable at the pinned SHA — a git object, so it cannot change, only disappear
if the repository is deleted.
