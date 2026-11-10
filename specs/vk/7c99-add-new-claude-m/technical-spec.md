# Technical spec — Add Claude Fable 5.2 and Mythos 5.2 (vk/7c99-add-new-claude-m)

## Request

"Add New Claude Models: Fable 5.2 and Mythos 5.2". The user chose **option B**
(2026-11-10): add the models *and* move the pinned Claude Code CLI to a release
that supports them. That explicit choice is the human release-note review the
`@anthropic-ai/claude-code` Renovate carve-out requires (`CLAUDE.md` →
Dependencies) and authorizes the hand bump.

## Upstream facts (verified 2026-11-10)

| Source | Fact |
| --- | --- |
| platform.claude.com models overview | `claude-fable-5-2`: 1M context, 128K output, adaptive thinking always on, effort `low`…`max`. |
| platform.claude.com Mythos 5.2 page | `claude-mythos-5-2`: same model/specs as Fable 5.2, available only to organizations verified through Anthropic programs (e.g. Cyber Verification Program). Released 2026-11-09. |
| code.claude.com model-config | Fable 5.2 requires Claude Code **≥ 2.1.321**; `fable` resolves to Fable 5.2 (Fable 5 in Claude apps gateway sessions). No `mythos` alias. |
| npm | `latest` = 2.1.322, `stable` = 2.1.311 (too old). |
| sadjow/claude-code-nix | `36dfedaa4451…` = "update claude-code to version 2.1.322" (#468). |

## Artifact evidence (2.1.322 native binary)

`@anthropic-ai/claude-code-linux-x64@2.1.322` tarball SHA-1
`b56040e7f01b956839d56504882d36a177f06a89` matches npm `dist.shasum`. Native
`claude` SHA-256 `bfb3703ad5380242328459bc62ada4bde8ae7ba57c6e1eeadb9c00e08874c478`.
The 2.1.281 binary re-downloaded for diffing hashes to the previously recorded
`56fe3da8…dce1`.

Embedded model catalog:

- `claude-fable-5-2` (family `fable`, 1M native, capabilities include `effort`,
  `xhigh_effort`, `max_effort`).
- `claude-mythos-5-2` (family `mythos`, first-party only, 1M native, same effort
  capabilities).
- `claude-sonnet-5-5`, `claude-haiku-5-5` (both 1M native, both with
  `effort`/`xhigh_effort`/`max_effort`).
- Alias table (first-party default):
  `opus → claude-opus-5-5`, **`sonnet → claude-sonnet-5-5`**,
  **`haiku → claude-haiku-5-5`**, **`fable → claude-fable-5-2`**.

**Consequence the request did not name:** bumping the CLI silently moves the
`sonnet` and `haiku` aliases too (exactly the risk the carve-out calls out).
VK's `model_aliases` must say what the pinned CLI runs, and a test requires
every alias target to be an advertised catalog model. Sonnet 5.5 and Haiku 5.5
therefore join the catalog; otherwise saved `sonnet`/`haiku` profiles would
resolve to an entry the picker does not list.

Invariants re-verified unchanged against 2.1.322:

| Invariant (claude.rs) | 2.1.322 evidence |
| --- | --- |
| `BASH_DEFAULT_TIMEOUT_MS`/`BASH_MAX_TIMEOUT_MS` semantics | `var a=120000,s=600000; kK` (default) / `kX` = `Math.max(requested, kK)` — identical logic. |
| `BACKGROUND_POLLER_TOOLS` canonical names | `Monitor`, `TaskOutput`, `TaskStop`, `CronCreate`, `CronDelete`, `CronList`, `ScheduleWakeup` present. |
| Alias → canonical map | `{Task:"Agent",KillShell:"TaskStop",KillBash:"TaskStop",ListPeers:"ListAgents",…}` unchanged (adds `Eval:"REPL"`, irrelevant). |
| Legacy TaskOutput set | `["…","TaskOutput","AgentOutputTool","BashOutputTool","AgentOutput","BashOutput"]` unchanged. |
| `PEER_TOOLS` | `ListAgents`/`ListPeers` constants and `SendMessage` present. |
| `crossSessionInbound: "refuse"` | Restrictive values `["refuse","hold"]`; `crossSessionInbound=refuse` path present. |
| `disabledMcpjsonServers` | Present. |
| `--resume` scans all `projects/*` | Re-ran the 9f5d experiment: empty → `No conversation found`; transcript under another project or under `$HOME/.claude` → lookup passes (`Not logged in`). |

Release notes 2.1.282–2.1.322 reviewed. Items relevant to VK: 2.1.321 Fable 5.2;
2.1.319 gateway auto-mode no longer falls back to the built-in classifier (VK
does not use gateway auto mode); 2.1.317 adds
`CLAUDE_CODE_DISABLE_NESTED_SESSION_ID` (opt-in, unused); 2.1.314 PreCompact
hook output capped at 10,000 chars (VK registers no PreCompact hook); 2.1.312+
resume fixes (compatible with the probe above). No wire-protocol break.

## Changes

### vibe-kanban (`crates/executors/src/executors/claude.rs`)

1. `base_command(false)` → `npx -y @anthropic-ai/claude-code@2.1.322`.
2. Catalog (`default_discovered_options`), versioned IDs, newest first per family:
   `claude-opus-5-5` Opus 5.5, `claude-opus-5` Opus 5, `claude-sonnet-5-5`
   Sonnet 5.5, `claude-sonnet-5` Sonnet 5, `claude-fable-5-2` Fable 5.2,
   `claude-fable-5-1` Fable 5.1, `claude-mythos-5-2` Mythos 5.2,
   `claude-haiku-5-5` Haiku 5.5, `claude-haiku-4-5` Haiku 4.5. Default stays
   `claude-opus-5-5`.
3. Effort options (`low`…`max`) for every entry except `claude-haiku-4-5`
   (predicate becomes "not Haiku 4.5" rather than substring families, so Mythos
   and Haiku 5.5 get effort).
4. `model_aliases`: `opus`/`opus[1m]` → `claude-opus-5-5`, `sonnet` →
   `claude-sonnet-5-5`, `fable` → `claude-fable-5-2`, `haiku` →
   `claude-haiku-5-5`. No `mythos` alias (the CLI has none).
5. `context_window_for_model`: add `haiku`, `claude-fable-5-2`,
   `claude-mythos-5-2`, `claude-sonnet-5-5`, `claude-haiku-5-5` to the 1M set.
6. Every "verified against 2.1.281" comment/assert message → 2.1.322, pointing at
   this spec.
7. Tests updated: catalog list, alias table, context windows, pin asserts.

Wiki pages naming 2.1.281 as the deployed pin are refreshed in the knowledge
stage.

### homelab (deployment companion)

The Think fleet host CLI comes from the `claude-code-nix` flake input (VK workers
install it as a prerequisite). Bump that input to `36dfedaa4451…` (2.1.322),
update `tests/claude-code.nix` and `docs/claude-code.md`.

## Non-goals

- No change to the default model (Opus 5.5) or to `opus` behaviour.
- Mythos 5.2 access is an account entitlement; VK only advertises the ID.
  A non-entitled account gets the API's error, as with any model.
- No new background/peer control; existing hooks stay the enforcement boundary.

## Acceptance

- Picker lists the nine versioned entries above; Fable 5.2 / Mythos 5.2 /
  Sonnet 5.5 / Haiku 5.5 show effort choices and seed a 1M context window.
- Saved `fable`/`sonnet`/`haiku` profiles resolve to the 5.2/5.5 entries.
- `SQLX_OFFLINE=true cargo test -p executors executors::claude --lib` passes;
  `pnpm run format`, `pnpm run check` pass; homelab
  `nix eval --impure --file tests/claude-code.nix` reports 2.1.322 for think1–6.
