# Prior knowledge — vk/7c99-add-new-claude-m

Knowledge bases searched (read-only): `vibe-kanban/wiki/` and
`homelab/docs/knowledge-base/`, plus precedent task artifacts
(`vibe-kanban/specs/vk/1f95-update-claude-mo/`, `homelab/specs/vk/129c-update-to-opus-5/`,
`vibe-kanban/specs/opus-5-5-verification.md`).

## Directly applicable

- **`wiki/executor-model-catalogs.md`**:
  - Executor discovery is the only source for the picker.
  - Advertise versioned IDs only, newest-first.
  - Guard the catalog with an exact ordered test over IDs, labels and effort IDs.
  - Claude upgrades must move the npm launcher, the catalog and the context
    mapping together.
  - Read `model_aliases` from the pinned native binary
    (`opus:"claude-…"`-style entries) and guard it with a pin-sensitive test.
  - Before moving the pin, inspect the native artifact for timeout keys and
    background-tool wire names, and record the checksum.
- **`homelab/docs/knowledge-base/claude-cli-launch-boundaries.md`**:
  - A host `pkgs.claude-code` bump does not change VK agents; VK launches its
    own npm pin.
  - The Think fleet shares the `claude-code-nix` flake input.
    `tests/claude-code.nix` checks all six Think hosts.
  - A CLI bump can move aliases without any settings change. Distinguish
    model support from account entitlement.
- **`wiki/model-picker-preferences.md`**: saved profiles may carry aliases. The
  picker resolves them through `resolveModelAlias`. Hidden-model filtering runs
  on the unfiltered catalog, so new entries need no frontend change.

## Pin-dependent claims to re-verify (each names 2.1.281 today)

| Page / code | Claim |
| --- | --- |
| `wiki/agent-process-lifecycle.md` | Native `ListAgents`/`ListPeers`/`SendMessage` disabled; `crossSessionInbound=refuse` settings file. |
| `wiki/mcp-pr-tools-and-connection-notices.md` | A same-name project `.mcp.json` entry replaces the user entry. Ancestor `.mcp.json` files load. `disabledMcpjsonServers` drops them. |
| `wiki/claude-missing-transcript-fallback.md` | `--resume <id>` searches every `projects/*` folder; the probe recipe uses a throwaway `CLAUDE_CONFIG_DIR`. |
| `claude.rs` | Bash timeout constants, `BACKGROUND_POLLER_TOOLS`, `PEER_TOOLS`, alias map, resume scan. |

## Precedent

- #272 (`vk/1f95`, Fable 5.1) and `vk/129c` (Opus 5.5) both hand-bumped the CLI
  together with the catalog change, with explicit user authorization. They
  recorded artifact evidence in a verification note and kept older explicit
  models.
- `vk/129c` also bumped the homelab `claude-code-nix` input and its
  `tests/claude-code.nix` version assert, and refreshed `docs/claude-code.md`.

## New in this task (not in any page yet)

- Claude Code 2.1.322 moves **`sonnet` → Sonnet 5.5** and **`haiku` → Haiku 5.5**,
  in addition to `fable` → Fable 5.2. Haiku is now 1M-context and supports
  effort, so the `haiku` alias leaves the 200K default.
- The CLI has a `mythos` family but **no `mythos` alias**. Mythos models are
  first-party only (no Bedrock, Vertex or Foundry IDs).
