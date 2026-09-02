# Research: Three rollout loose ends

## i18n comparison

`get_json_keys` already sorts with `LC_ALL=C sort -u`, but command substitution
followed by `printf "%s\n" "$keys"` does not make that ordering contract obvious
to `comm` and the current report confirms warnings. Normalize immediately at
each `comm` operand (or compare temporary normalized files) under `LC_ALL=C` so
producer and consumer share one locale. Keep jq parse errors observable rather
than converting them to an empty success.

All supported `common.json` files already use `_one` and `_other`; adding those
two keys is consistent with the project's current i18next resources. Translation
placeholders remain byte-identical inside each localized string.

## Background-helper response boundary

`start_background_helper` has two rejection sites: the direct empty-script
check and the shared `prepare_helper_start` result used for invalid directories
and the shared helper limit. Both currently call `error_with_data`, whose
response message is `None`. The same file's `start_poller_error_message` and
envelope test are the exact implementation template.

A bounded search found many server `error_with_data` callers. The MCP task server
uses workspace execution routes for helper/poller tooling; the requested helper
route is the confirmed MCP-reachable defect. Broader endpoint-by-tool tracing is
best handled as a separate audit rather than modifying unrelated API semantics.

## Codex setting history and strict mode

Commit `7c10c00d` introduced `include_apply_patch_tool` during the app-server
migration as a typed V1 `NewConversationParams` field. During the later V2
protocol migration, the field disappeared from `ThreadStartParams` and VK moved
the old setting into the generic config override map. Pinned Codex 0.144.1 has no
matching configuration field, so that move preserved syntax but not behavior.
No verified current equivalent exists.

The setting is still generated into `shared/types.ts` and
`shared/schemas/codex.json` from the Rust `Codex` struct. Removing the dead public
field requires regeneration through the repository command, never hand edits.

The Cargo checkout resolved for the `rust-v0.144.1` pin defines
`--strict-config` on app-server. Its upstream integration test
`app-server/tests/suite/strict_config.rs` proves an unknown config key makes the
process fail and names the key. `ConfigManager` also passes the strict flag into
each thread config builder, covering request overrides. Therefore the current
verified general control is to launch `app-server --strict-config` and pin that
exact command with a VK unit test.

## Dependencies

No new dependency is required.
