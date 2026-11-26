# Verification: Align Codex Model Catalog

## Results (2026-11-25)

| Command | Result |
| --- | --- |
| `cargo test -p executors discovered_models_match_current_chatgpt_catalog` (before catalog edit) | Failed as intended (old 6-entry catalog vs 10-entry contract) |
| same, after catalog edit | Passed: 1/1 |
| `cargo test -p executors` | Passed: 369 passed, 0 failed, 1 intentional ignored |
| `pnpm run generate-types:check` | Passed: 10 schemas, `shared/types.ts` up to date |
| `pnpm run format` | Passed, no formatter changes |
| `git diff --check` | Passed |

## Runtime evidence

| CLI | IDs | Result |
| --- | --- | --- |
| 0.171.0 | gpt-6.2-astra, gpt-6.1-terra | OK |
| 0.155.1 (deployed `VIBE_CODEX_COMMAND`) | gpt-6.2-astra, gpt-6.2-sol, gpt-6.1-sol, gpt-6.1-terra | 400 `model_not_found` (re-checked 2026-11-25: `model_not_in_accessible_catalog`) |
| 0.155.1 | gpt-6-astra (control) | OK |

## Scope

- Product code: `crates/executors/src/executors/codex.rs` only (catalog + test).
- Docs: `wiki/executor-model-catalogs.md`, `wiki/INDEX.md`, constitution
  review note, this spec directory.
- Homelab: not modified.

## Post-upgrade runtime (2026-11-25)

| Check | Result |
| --- | --- |
| VK executor (`Codex::spawn` + `spawn_follow_up`, protocol crates 0.144.1) vs 0.171.0, `gpt-6.2-sol` | Both turns succeeded on the same thread. A 0.155.1 control run behaved identically, with no new decode warnings |
| Nix-built think `pkgs.codex` 0.171.0 `model/list` | All 10 visible IDs present |
| Deployed `VIBE_CODEX_COMMAND` (think1) | `codex-0.171.0`. `gpt-6.2-astra`, `gpt-6.2-sol`, `gpt-6.1-sol` OK |
