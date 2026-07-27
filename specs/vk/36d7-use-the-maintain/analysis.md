# Analysis: pinned Slack MCP connector from the maintained fork

Cross-check of `spec.md`, `plan.md`, `tasks.md`, `contracts.md`,
`data-model.md`, `research.md` against
`.specify/memory/constitution.md` (v0.12.0). Findings are ordered by severity;
each names the artifact it concerns. Fixes applied after this report are noted
inline as **[fixed]**.

## Errors

- **E1 — `npm pack` output name contradicts the pinned URL.**
  Artifact: `contracts.md` §3, `data-model.md` §1, `tasks.md` T002/T005/T009.
  The launcher is named `@davidvasandani/slack-mcp-server-vk`, but the pinned
  asset is `slack-mcp-server-vk-1.3.0-vk.1.tgz`. `npm pack` on a **scoped**
  package emits `davidvasandani-slack-mcp-server-vk-1.3.0-vk.1.tgz`; uploading
  that file and pinning the other name yields a 404 at install time — the exact
  failure the feature exists to prevent. Resolution: drop the scope (package
  name `slack-mcp-server-vk`), so `npm pack` produces the pinned filename
  directly and no rename step can drift. **[fixed]**

- **E2 — the binary has no `--version` flag.**
  Artifact: `plan.md` (Technical Context), `tasks.md` T008/T018.
  `cmd/slack-mcp-server/main.go` registers only `-t/--transport`,
  `-e/--enabled-tools` and `--no-cache`; there is no version flag, so
  "confirm each binary reports `v1.3.0-vk.1` via `--version`" is not executable
  and would be silently skipped or fudged. The stamped `version.Version` *is*
  observable: `pkg/server/server.go` passes it to `server.NewMCPServer`, so it
  comes back in the MCP `initialize` response as `serverInfo.version`. Use that
  (plus `strings <binary> | grep` as a cross-check for the non-native
  platforms, which cannot be executed here at all). **[fixed]**

- **E3 — FR-12 has an acceptance criterion but no task.**
  Artifact: `spec.md` (FR-12, AC "`git diff` touches only the Slack entry`") vs
  `tasks.md`. Nothing verifies that the other seven catalog entries are
  untouched. A one-line `git diff` assertion belongs in the verification phase;
  cheap, and it is the only guard against an accidental reformat of a file that
  is `include_str!`-embedded into every agent's config. **[fixed: added T022a]**

## Warnings

- **W1 — Renovate will not offer `-vk.N` releases as configured.**
  Artifact: `plan.md` §4 manager snippet, `tasks.md` T015.
  `v1.3.0-vk.1` is a semver **prerelease**. Renovate's default
  `ignoreUnstable: true` filters prereleases out, so the manager would match the
  current value and then never propose an update — a silent no-op that reads
  like coverage. The `packageRules` entry must set `ignoreUnstable: false` and
  an explicit `versioningTemplate` (`semver`), and the manager needs
  `extractVersionTemplate` consistent with the `v` prefix. **[fixed in plan]**

- **W2 — the launcher's tests have no runner.**
  Artifact: `tasks.md` T006. A test file in the fork with no CI hook and no
  `npm test` wiring will rot. Either add `"scripts": {"test": "node --test test/"}`
  to the launcher's `package.json` and run it in `build-release.sh` before
  packing, or drop the task. Recommended: wire it into the build script so a
  broken launcher cannot be released. **[fixed: T005 now runs the tests]**

- **W3 — outward-facing publish step has no guard.**
  Artifact: `tasks.md` T007–T009, `plan.md` Risks.
  Pushing a commit and tag to `davidvasandani/slack-mcp-server` `master` and
  creating a public release is user-visible and only partially reversible
  (a deleted release/tag can still have been fetched). The plan notes that the
  fork's `release.yaml` triggers on *any* tag push and ends in
  `make npm-publish` against the **upstream** package name; nothing in the task
  list disarms that before T007. Add an explicit pre-flight: confirm the fork's
  Actions state, guard or disable the tag-triggered workflow, then tag.
  **[fixed: added T006a]**

- **W4 — `SLACK_MCP_SERVER_VK_CACHE_DIR` is contract-only.**
  Artifact: `contracts.md` §2 and `tasks.md` T018 use it; `spec.md` and the
  documentation task (T016) do not mention it. Either document both env vars or
  mark the cache override as test-only. Recommended: document it — an operator
  debugging a stale cache needs it. **[fixed: T016 scope widened]**

- **W5 — the negative-permission check lacks a subject.**
  Artifact: `spec.md` AC, `tasks.md` T021. "A file the connected identity cannot
  read" is not identified, and manufacturing one is not trivially available. A
  deleted/nonexistent ID exercises `file_not_found`, not `access_denied` — a
  different branch. Acceptable resolution: assert the mapped error path with a
  well-formed but non-existent ID, and record explicitly that
  `access_denied` was not exercised end-to-end rather than implying it was.
  **[fixed: T021 restated]**

- **W6 — cross-platform assets are unverifiable from this host.**
  Artifact: `tasks.md` T008/T018. Only `linux-x64` can be executed here; the
  other five are verified by digest and `strings` only. That is a real coverage
  limit and should be stated in the completion report rather than left implied.

## Info

- **I1 — no new dependencies.** `crates/executors/Cargo.toml` already carries
  `sha2 = "0.10"` and workspace `reqwest`, satisfying the constitution's
  "no new top-level dependencies without recording the reason" constraint
  without needing a research note.
- **I2 — `1.3.0-vk.1` is valid semver** (prerelease identifier `vk.1`), so npm
  accepts it in `package.json`; it is only Renovate's stability filter that
  needs the explicit opt-in (W1).
- **I3 — frontend is genuinely untouched.**
  `packages/web-core/src/shared/lib/sharedMcpSettingsState.test.ts` mentions
  `slack-mcp-server`, but as a local fixture object, not as an assertion about
  the catalog. No frontend task is missing.
- **I4 — constitution coverage.** Principle XV (added this session) is
  enforced by T013; VIII by the launcher's digest + atomic-rename behaviour;
  II by T012–T014 and T006; VI by reusing the `cli_tools` pinning idiom and
  `mcp_test.rs` instead of new machinery. No deviations found.
- **I5 — the `-e/--enabled-tools` flag exists** alongside
  `SLACK_MCP_ENABLED_TOOLS`; the acceptance check should use the **env var**,
  since that is what the spec and the catalog entry's contract talk about.

## Post-implementation: independent review (Codex CLI)

Two passes over the implemented diff, the launcher, and the release script.

| # | Severity | Finding | Outcome |
| --- | --- | --- | --- |
| R1 | High | Nothing verifies the launcher tarball's digest at install time: `npx` fetches it and npm has no integrity flag for URL specs, so a replaced release asset could ship a malicious `bin` *and* a matching binary-checksum table. | **Accepted as inherent, mitigated.** VK writes command lines; it never installs MCP servers, so it cannot verify what npm fetches. Added `.github/workflows/pinned-artifacts.yml` — a weekly scheduled job running the ignored digest test — and documented the residual risk at the constant, in the docs, and in the knowledge-base page. Detection moved from "if someone runs a test" to "within a week". |
| R2 | Medium | The Renovate manager's `extractVersionTemplate` (`^v(?<version>.*)$`) allegedly conflicts with a `currentValue` captured without the `v`, so no update PR would open. | **Rebutted, and the rebuttal was confirmed on the second pass.** `extractVersion` normalises the *datasource's* versions (`github-releases` tag names), not the file's `currentValue`; both sides therefore compare as `1.3.0-vk.2`. Added that reasoning to the manager's `description` so the next reader does not repeat the analysis. |
| R3 | Low | `git show --date=format-local` renders in the builder's timezone while the format appends a literal `Z`, so the same tag stamps different `BuildTime` values — and different digests — on non-UTC machines. | **Confirmed and fixed.** `TZ=UTC0` pinned for that command; fork release **v1.3.0-vk.2** cut with the corrected script; VK re-pinned to it; reproducibility re-verified by building the same tag under `TZ=Asia/Tokyo` and `TZ=America/Los_Angeles` and comparing checksums (identical). v1.3.0-vk.1 was left published, unmodified, and annotated as superseded. |
| R4 | Medium | The new workflow and knowledge-base page were untracked, so they were missing from the reviewed `git diff`. | **Artefact of the review input**, not of the change: both files exist and are now `git add -N`-tracked so they appear in diffs. |
