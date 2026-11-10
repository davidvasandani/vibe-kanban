# Implementation plan — vk/7c99-add-new-claude-m

Inputs: `technical-spec.md` and `prior-knowledge.md` in this folder.

## Step 1 — Re-verify the remaining pin-dependent behaviours (evidence only)

Already done in the spec: bash timeouts, tool names and aliases, legacy set,
`crossSessionInbound`, `--resume` scope.

Remaining: run the MCP experiment from `wiki/mcp-pr-tools-and-connection-notices.md`
against the 2.1.322 binary, using a throwaway `HOME`:
- a user entry and a same-name project `.mcp.json` entry, checked with `claude mcp list`;
- an ancestor `.mcp.json`;
- `--settings '{"disabledMcpjsonServers":[…]}'`.

Record the results in `verification.md`.

## Step 2 — Launcher pin (`claude.rs::base_command`)

`2.1.281` → `2.1.322`.

## Step 3 — Catalog, effort and aliases (`default_discovered_options`)

- Replace the model list with nine versioned entries, newest-first within each
  family. Families are ordered Opus, Sonnet, Fable, Mythos, Haiku.
- Replace the substring `supports_effort` closure with "every entry except
  `claude-haiku-4-5`".
- Alias map:
  - `opus` and `opus[1m]` → `claude-opus-5-5`
  - `sonnet` → `claude-sonnet-5-5`
  - `fable` → `claude-fable-5-2`
  - `haiku` → `claude-haiku-5-5`
- Update the comment to say 2.1.322.

## Step 4 — Context windows (`context_window_for_model`)

Add `haiku`, `claude-sonnet-5-5`, `claude-fable-5-2`, `claude-mythos-5-2` and
`claude-haiku-5-5` to the 1M set. `claude-haiku-4-5` stays at the 200K default.

## Step 5 — Verification-source comments

Update every 2.1.281 reference (lines ~291, 384, 534, 562, 856, 1411) to
2.1.322. Point to `specs/vk/7c99-add-new-claude-m/verification.md` and keep the
historical 2.1.268 provenance.

## Step 6 — Tests (`claude.rs` tests module)

- `test_claude_discovered_catalog_is_versioned_and_defaults_to_opus_5_5`: exact
  nine-entry list. Haiku 4.5 has no efforts; Haiku 5.5 is 1M.
- `test_claude_legacy_aliases_resolve_to_catalog_models`: new alias table and
  2.1.322 pin message.
- `test_context_window_for_model`: add `haiku` and the new IDs.
- Background-tool pin test: 2.1.322 message.

## Step 7 — Homelab companion

- Update `flake.lock` `claude-code-nix` to `36dfedaa4451…` with
  `nix flake update claude-code-nix`. Confirm the locked rev and narHash.
- `tests/claude-code.nix`: assert `2.1.322`.
- `docs/claude-code.md`: rewrite for Fable 5.2 / Mythos 5.2 / 2.1.322, including
  the alias moves.
- `docs/knowledge-base/claude-cli-launch-boundaries.md`: note the 2.1.322 alias
  moves (stage 12).

## Step 8 — Validate

- `pnpm install --frozen-lockfile`, then `pnpm run format`.
- `SQLX_OFFLINE=true cargo test -p executors executors::claude --lib`.
- `cargo clippy -p executors`.
- `pnpm run check` if time allows; frontend code is untouched.
- Homelab: `nix eval --impure --file tests/claude-code.nix`, falling back to
  `nix eval` of the input's version if evaluating the host closures is too
  heavy.

## Step 9 — Review, knowledge, PRs

- Run the Codex review over both diffs.
- Update the wiki: `executor-model-catalogs`, `agent-process-lifecycle`,
  `mcp-pr-tools-…`, `claude-missing-transcript-fallback`, `INDEX.md`. Update the
  homelab KB page.
- Open and merge a PR in each repo: VK first, then homelab.

## Risks

- **Alias drift for saved `sonnet`/`haiku` profiles:** intended (it is what the
  CLI runs) and visible in the picker. Called out in the PR body.
- **Mythos entitlement:** most accounts get an API error. The label is
  explicit, and the user requested it.
- **A day-old `latest` CLI:** mitigated by re-verifying the artifact and by
  reviewing the release notes (user-authorized).
