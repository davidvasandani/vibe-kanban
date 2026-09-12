# Tasks: Gmail MCP connector with multi-account instances

**Plan**: `./plan.md`

Two independent slices — **A** (catalog entry, Rust + JSON) and **B**
(multi-instance, TypeScript) — touch disjoint files and can proceed in parallel
after Phase 1. Docs (Phase 4) depend on both being settled.

## Phase 1: Setup

- [x] T001 Put the toolchain on `PATH` and install dependencies:
      `export PATH="$HOME/.cargo/bin:$PATH"`, `corepack enable pnpm`,
      `pnpm install --frozen-lockfile`. Confirm `cargo --version` and
      `pnpm --version` answer. Every later verification fails without this, for
      reasons unrelated to the change.

## Phase 2: Core — Slice A (catalog entry)

- [x] T002 Add the `gmail` server entry to `crates/executors/default_mcp.json`
      as a sibling of `slack`. Full 40-hex commit SHA in the install spec;
      `--tool-prefix=YOUR_PREFIX_` in `args`; `GMAIL_CREDENTIALS_PATH:
      "/absolute/path/to/credentials.json"` in `env`; **no** `GMAIL_OAUTH_PATH`. Exact shape
      in `contracts/README.md` C-2.
- [x] T003 Add the `meta.gmail` block to the same file's `meta` object — `name`,
      `description`, `url` pointing at the fork. No `icon` key (clarification
      C5). The `url`'s `owner/repo` must match the install spec's; T005 asserts
      it. (Same file as T002 — serial with it.)

## Phase 2: Core — Slice B (multi-instance)

- [x] T004 [P] Add the exported pure function `nextAvailableServerName(key,
      existing)` to `packages/web-core/src/shared/lib/sharedMcpSettingsState.ts`,
      beside `preconfiguredMcpServers`. Contract and behaviour table in
      `contracts/README.md` C-1. Separator is `_`, not a space or parenthesis.

## Phase 3: Wiring

- [x] T005 Add Rust tests to `crates/executors/src/mcp_config.rs` `mod tests`,
      beside the Slack tests (depends on T002, T003):
      - a `GMAIL_MCP_FORK_REVISION` module constant holding the pinned SHA, so
        the pin has one named home;
      - a `parse_github_git_spec(spec) -> Option<(owner, repo, commit_ish)>`
        helper beside the existing `parse_github_release_asset` (`:654`);
      - `gmail_preconfigured_server_matches_the_documented_stdio_contract`
        (model on `:635`);
      - `gmail_preconfigured_server_pins_an_immutable_fork_revision` — a **shape**
        test (model on `:666`): spec parses; `owner/repo` equals the owner/repo
        parsed from `meta.gmail.url`; `commit_ish == GMAIL_MCP_FORK_REVISION` and
        is 40 lowercase hex; rejects `#main`, `#master`, `refs/heads/`,
        `@latest`, and a fragment-less bare repo reference;
      - `gmail_preconfigured_server_adapts_for_codex_and_opencode` (model on
        `:723`) — Codex keeps `env`; Opencode gets `type:"local"`, a `command`
        **array**, and `environment`.

      Do **not** add a SHA-256 constant or a `pinned-artifacts.yml` job — see
      `research.md` R3.

- [x] T006 Wire `nextAvailableServerName` into `addPreconfigured` at
      `packages/web-core/src/shared/dialogs/settings/settings/McpSettingsSection.tsx:609-627`
      (depends on T004). Allocate against `draft.servers.map(s => s.name)`
      **before** calling `setServer`. Add `draft.servers` to the `useCallback`
      dependency array (or use the functional form) — a stale closure hands out
      one name twice, and `setServer` de-duplicates by name, so the collision
      destroys the first instance silently rather than erroring.

- [x] T007 Stop disabling the catalog tile in the same file at `:1104-1121`
      (depends on T006). Keep computing `added` — it still drives the check mark
      and dimmed styling — but drop `disabled={added}` and the `cursor-default`
      branch of the `cn(...)` call. Only "looks added" survives; "is inert" goes.

## Phase 4: Validation

- [x] T008 [P] Add TypeScript tests to
      `packages/web-core/src/shared/lib/sharedMcpSettingsState.test.ts`
      (depends on T004): the five `nextAvailableServerName` cases from
      `contracts/README.md` C-1 including the gap case (`['gmail','gmail_3']` →
      `gmail_2`); a property test that every generated name matches
      `/^[a-zA-Z0-9_-]+$/`; and "adding a template twice yields two distinct
      draft servers".

- [x] T009 [P] Add the **Gmail connector** section to
      `docs/integrations/mcp-server-configuration.mdx`, after the Slack section
      (depends on T002, T003). Mintlify conventions, British spelling, second
      person. Must cover, in order:
      1. Prerequisites — a Google Cloud OAuth client at
         `~/.gmail-mcp/gcp-oauth.keys.json`, plus `git` and network access; the
         first launch builds from source and is slow (~53 s measured).
      2. The entry as shipped, with both placeholders called out.
      3. One-off consent **per mailbox**, run in a terminal with
         `GMAIL_CREDENTIALS_PATH` set and **without** `--tool-prefix`.
      4. A `<Warning>`: each instance needs its own `--tool-prefix`, or some
         clients dedupe tools by base name and one instance silently answers for
         the other.
      5. A worked three-account table using **neutral** names
         (`gmail_personal`, `gmail_work`, `gmail_client`) — never the requester's
         employer or client names. Columns: server name,
         `GMAIL_CREDENTIALS_PATH`, `--tool-prefix`.
      6. That 28 tools per instance adds up, cross-referencing the existing
         "Limit MCP Servers" tip, with `--scopes` as the narrowing lever.
      7. That a missing OAuth client makes "Test connection" report `failed`, and
         that a present-but-unconsented account tests **green** and fails at
         first use — the two setup steps carry equal weight.

- [x] T010 [P] Add a line to the **Popular MCP Servers** intro in the same doc
      noting a template can now be added more than once (depends on T007) — a
      user-visible behaviour change.

- [x] T011 [P] Add a Gmail bullet to **Dependencies** in `AGENTS.md`, after the
      Slack one: the pin is a fork **commit SHA** outside Renovate's reach and is
      bumped **by hand**, moving the SHA in `default_mcp.json`, the
      `GMAIL_MCP_FORK_REVISION` constant in `crates/executors/src/mcp_config.rs`,
      and the revision named in the docs page **together**; unlike Slack there is
      no digest constant and no audit job, because a git SHA is immutable and npm
      verifies it at install time. Edit `AGENTS.md` only — `CLAUDE.md` is a
      symlink to it.

## Phase 5: Verification

- [x] T012 `cargo test -p executors gmail` (depends on T005).
- [x] T013 `cargo test -p executors` — no Slack or adapter regressions.
- [x] T014 `pnpm --filter @vibe/web-core test` (depends on T008). The package is
      `@vibe/web-core`, not `@vibe-kanban/web-core`; its `test` script is
      `vitest run`.
- [x] T015 `pnpm run check` — frontend plus all Rust workspaces.
- [x] T016 `pnpm run lint`.
- [x] T017 `pnpm run format` — required before completing the task.
- [x] T018 Confirm `pnpm run generate-types:check` reports **no** diff. No Rust
      type changed, so a diff means something unintended happened — investigate
      rather than committing a regenerated file.

## Phase 6: Manual verification

- [x] T019 **Done before implementation.** Cold-cache install of the pinned git
      spec: 53 s, exit 0, executable `dist/index.js` (78 KB). `prepare` runs.
- [x] T020 **Done before implementation.** Driven over stdio: `initialize`
      returned `serverInfo {"name":"gmail","version":"1.0.0"}`; `tools/list`
      returned **28** tools (README's 26 omits `get_thread` and
      `list_inbox_threads`), every one prefixed by `--tool-prefix=personal_`.
      Also established: the server **exits before `initialize`** without
      `gcp-oauth.keys.json`, and serves `tools/list` with a merely *present*
      (not valid) keys file and no `credentials.json`.
- [ ] T021 Two instances in the running app: Settings → MCP Servers → add Gmail
      twice; confirm `gmail` and `gmail_2` as separate cards; rename both, give
      each its own credentials path and prefix; save; confirm two distinct keys
      in the assigned agent's native config file.
- [ ] T022 Two real mailboxes: authorise two accounts and confirm each prefixed
      tool set reads its own mailbox.

T021 needs a running app; T022 additionally needs real Google credentials. If
either cannot be completed in this environment, **say so explicitly in the task
summary** rather than implying it passed.

## Phase 7: Knowledge distillation (pipeline stage 12)

- [ ] T023 Record the reusable knowledge: multi-instance catalog templates
      (instance-name allocation, the generator↔validator binding, why per-account
      credentials require per-account names); client-side tool-name dedupe as a
      silent failure mode and `--tool-prefix` as its remedy; and git-commit-SHA
      pinning versus release-asset pinning, including which integrity layer it
      removes the need for.
- [ ] T024 Amend `docs/knowledge-base/forked-mcp-server-packaging.md` — its
      rejected-alternatives row for `npx github:owner/repo#<sha>` should note the
      objection is Go-specific and does not apply to a package with a `prepare`
      script.
- [ ] T025 Correct the stale claim in
      `docs/knowledge-base/shared-mcp-configuration.md` that "the current shared
      MCP settings UI does not render catalog suggestions" — it has rendered them
      since `McpSettingsSection.tsx:1094-1161`.
- [ ] T026 Tag the touched knowledge pages with `vk/4daf-gmail-mcp` and refresh
      the relevant `INDEX.md`.

<!--
Conventions:
- `T001` … task ids are stable and referenced by the dependency graph.
- `[P]` … parallel-safe (independent files). Omit for tasks that must be serial.
- `[ ]` / `[x]` … completion checkbox, toggled from the workbench.
-->
