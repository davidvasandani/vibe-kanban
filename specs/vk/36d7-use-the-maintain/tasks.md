# Tasks: pinned Slack MCP connector from the maintained fork

**Plan**: `./plan.md`

Paths starting with `fork:` are in a scratch clone of
`davidvasandani/slack-mcp-server`; all other paths are in this repository
(relative to the `vibe-kanban/` repo root).

## Phase 1: Fork artifact — build inputs

- [x] T001 Clone `davidvasandani/slack-mcp-server` into a scratch dir; assert
      `git merge-base --is-ancestor 04633fb892dc6dd38c3faffe29ff9b30829560c6 HEAD`,
      that `pkg/handler/attachment.go` exists and that
      `ToolAttachmentGetData` is registered ungated in `pkg/server/server.go`.
      Run `go build ./...` and `go test ./pkg/handler/... ./pkg/server/...`.
- [x] T002 [P] Add `fork:packaging/npm-launcher/package.json` — name
      `slack-mcp-server-vk` (**unscoped**, so `npm pack` emits exactly
      `slack-mcp-server-vk-1.3.0-vk.1.tgz`, the pinned asset name — analysis
      E1), version `1.3.0-vk.1`, single `bin` `slack-mcp-server`,
      `"scripts": {"test": "node --test test/"}`, `files` allow-list, zero
      dependencies, repository = the fork.
- [x] T003 [P] Add `fork:packaging/npm-launcher/bin/slack-mcp-server.js` — the
      resolve → cache → download → verify → exec launcher per
      `../contracts.md` §2 (escape hatch, platform map, digest enforcement,
      stdio inheritance, signal forwarding, exit-code mirroring, stderr-only
      diagnostics).
- [x] T004 [P] Add `fork:packaging/npm-launcher/README.md` — what the package
      is, why it is not on npm, and how to cut the next release.
- [x] T005 Add `fork:scripts/build-release.sh` — `make build-all-platforms`,
      compute per-asset SHA-256, write `packaging/npm-launcher/checksums.json`
      (schema in `../data-model.md` §1), **run the launcher tests
      (`npm test`) before packing** so a broken launcher cannot be released
      (analysis W2), `npm pack` the launcher, emit `checksums.txt` (depends on
      T002–T004, T006).
- [x] T006 [P] Add `fork:packaging/npm-launcher/test/launcher.test.mjs` — a
      dependency-free `node --test` suite covering unsupported platform, digest
      mismatch, cache hit, `SLACK_MCP_SERVER_VK_BINARY` passthrough, argv
      forwarding, and exit-code mirroring (depends on T003).
- [x] T006a Pre-flight the fork before any tag exists: check
      `GET /repos/{fork}/actions/permissions` and disarm the tag-triggered
      `.github/workflows/release.yaml` (whose last step `make npm-publish`
      targets the **upstream** npm package name) so tagging cannot publish
      anything (analysis W3).

## Phase 2: Fork artifact — publish

- [x] T007 Commit Phase 1 on the fork's `master`; create annotated tag
      `v1.3.0-vk.1` on that commit (superseded during review by `v1.3.0-vk.2`) (depends on T001–T006a).
- [x] T008 Run `fork:scripts/build-release.sh` at the tag. The binary has **no
      `--version` flag** (analysis E2): confirm the stamped version with
      `strings <binary> | grep v1.3.0-vk.1` for all six, and, for the
      natively-runnable one, via the MCP `initialize` response's
      `serverInfo.version` (depends on T007).
- [x] T009 Publish GitHub release `v1.3.0-vk.1` with the six binaries, the
      launcher tarball and `checksums.txt`; release notes name the upstream base
      version, fork PR #1 and merge commit `04633fb` (depends on T008).
- [x] T010 Record the pins for Phase 3: launcher tarball URL + SHA-256, the six
      binary SHA-256s, and the stamped version string (depends on T009).

## Phase 3: Repoint Vibe Kanban

- [x] T011 Update the `slack` entry's `args[1]` in
      `crates/executors/default_mcp.json` to the pinned release-asset URL;
      change nothing else in the file (depends on T010).
- [x] T012 Add `SLACK_MCP_FORK_TAG` and `SLACK_MCP_LAUNCHER_SHA256` constants
      and update `slack_preconfigured_server_matches_the_documented_stdio_contract`
      and `slack_preconfigured_server_adapts_for_codex_and_opencode` in
      `crates/executors/src/mcp_config.rs` (depends on T011).
- [x] T013 Add `slack_preconfigured_server_pins_an_immutable_fork_artifact` to
      `crates/executors/src/mcp_config.rs` — URL shape, owner/repo equality with
      `meta.slack.url`, tag equality with `SLACK_MCP_FORK_TAG`, and rejection of
      `@latest` / `#master` / `refs/heads/` / `/archive/` (depends on T012).
- [x] T014 Add `#[ignore]`d `slack_pinned_launcher_matches_recorded_digest` to
      `crates/executors/src/mcp_config.rs` using the crate's existing `reqwest`
      + `sha2` (depends on T012).

## Phase 4: Update process and documentation

- [x] T015 [P] Add the Slack fork-release custom manager and its
      no-auto-merge / `needs-review` `packageRule` to `renovate.json`. The rule
      MUST set `ignoreUnstable: false` and an explicit semver
      `versioningTemplate`, or Renovate silently never proposes a `-vk.N`
      prerelease (analysis W1) (depends on T011).
- [x] T016 [P] Document the connector in
      `docs/integrations/mcp-server-configuration.mdx`: installed fork
      version/revision, `attachment_get_data` on by default,
      `SLACK_MCP_ENABLED_TOOLS` exclusion, both launcher env vars
      (`SLACK_MCP_SERVER_VK_BINARY`, `SLACK_MCP_SERVER_VK_CACHE_DIR` — analysis
      W4), and the release-cutting checklist (depends on T010).
- [x] T017 [P] Add the Slack-pin line to the Dependencies section of
      `CLAUDE.md` (depends on T011).

## Phase 5: Verification

- [x] T018 Cache-isolated artifact check: `npm_config_cache` and
      `SLACK_MCP_SERVER_VK_CACHE_DIR` pointed at fresh temp dirs; run the pinned
      spec through `npx`; record launcher version, resolved binary path, its
      SHA-256 and `--version` (depends on T011).
- [x] T019 Protocol check: drive the launched server over stdio (`initialize`,
      `notifications/initialized`, `tools/list`); assert `attachment_get_data`
      present; repeat with `SLACK_MCP_ENABLED_TOOLS` excluding it and assert
      absent (depends on T018).
- [x] T020 Live Slack check with the connected `SLACK_MCP_XOXP_TOKEN`: search
      `https://sweetgreen.slack.com/archives/C0BE62MCDU6/p1784648794618929` →
      assert attachment id `F0BJX4Y3N5A`; call `attachment_get_data` for that id
      → assert the retrieval handler returns metadata + content (depends on
      T019).
- [x] T021 Negative check: call `attachment_get_data` for a well-formed but
      non-existent file ID → assert the mapped Slack-origin error
      (`file_not_found`), and record explicitly that `access_denied` was **not**
      exercised end-to-end because no unreadable-but-existing file is available
      to this identity (analysis W5) (depends on T019).
- [x] T022 Repository checks: `pnpm install --frozen-lockfile`,
      `cargo test -p executors`, `pnpm run check`, `pnpm run lint`,
      `pnpm run format` (depends on T011–T017).
- [x] T022a Assert the catalog diff is Slack-only: `git diff` on
      `crates/executors/default_mcp.json` touches only the `slack` entry's
      `args` (FR-12, analysis E3) (depends on T011).
- [x] T023 Run the ignored digest test deliberately:
      `cargo test -p executors slack_pinned_launcher_matches_recorded_digest -- --ignored`
      (depends on T014, T009).

## Phase 6: Review and knowledge capture

- [x] T024 Independent Codex review of the diff; address confirmed findings and
      re-verify (depends on T022). Two passes; see `analysis.md` §Post-implementation.
      Fixing the confirmed TZ finding required cutting fork release **v1.3.0-vk.2**
      and re-pinning VK to it, which exercised the documented update process
      end to end.
- [x] T025 Add `docs/knowledge-base/forked-mcp-server-packaging.md` and its
      `docs/knowledge-base/INDEX.md` row, plus a cross-reference from
      `shared-mcp-configuration.md` (depends on T024).
