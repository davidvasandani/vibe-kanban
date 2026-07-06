# Implementation Plan: First-Class MCP Server Configuration UX

See `SPEC.md` for the full design. All changes are in `packages/web-core` (frontend only).

## Step 1 — Server-entry codec module ✅

**New file:** `packages/web-core/src/shared/lib/mcpServerCodec.ts`
- `McpTransport`, `McpServerFormValues`, `McpServerCodec` types.
- Shared helpers: `pairsToRecord`, `argsFromLines`, strict type guards.
- Codecs: claude-style (Claude/Amp/Droid/Copilot), cursor, gemini (Gemini/Qwen), codex (stdio-only), opencode (local/remote).
- `codecForAgent(agent)` registry; `transportOf(codec, entry)` helper.
- `serialize(values, original)` preserves unrecognized keys and drops stale keys on transport switch.

**New file:** `mcpServerCodec.test.ts` — round-trips per codec, unknown-key preservation, parse rejections, Opencode command split/join, Gemini `httpUrl`, Cursor typeless URL, transport-switch key dropping.

## Step 2 — Add/edit dialog ✅

**New file:** `packages/web-core/src/shared/dialogs/settings/settings/McpServerDialog.tsx`
- NiceModal component; props `{codec, existingNames, initial?}`; resolves `{name, entry} | undefined`.
- Fields: name, transport select, command, args textarea (one per line), env rows, url, header rows.
- Custom-entry JSON mode when `parse` returns `null`.
- Inline validation; reusable `KeyValueRows` sub-component.

## Step 3 — Rework `McpSettingsSection` ✅

**Edit:** `McpSettingsSection.tsx`
- Object-based `servers` state + `originalSnapshot`; dirty = stringified inequality.
- Server list cards with transport badge + summary + edit/remove; empty state; "Add server".
- Popular servers grid inserts `preconfigured[key]`; check state when already added.
- JSON escape hatch (`mode: 'form' | 'json'`) reusing `McpConfigStrategyGeneral`.
- Save posts `{servers}` directly.

## Step 4 — i18n ✅

**Edit:** `en/settings.json` — add `settings.mcp.list.*`, `settings.mcp.dialog.*`, `settings.mcp.validation.*`, `settings.mcp.json.*`, `labels.servers`, `labels.serverHelperForm`. Other locales fall back to English.

## Step 5 — Verify

1. `vitest run` — codec tests green.
2. `pnpm run check`, `pnpm run lint` (and unused-i18n-key check).
3. Manual pass with `pnpm run dev` (optional): per-agent load, add/edit/remove, popular add, JSON toggle round-trip, save & inspect written config file.
4. `pnpm run format`.

## Step 6 — Review & PR (pipeline stages 3–4)

1. Codex review of the diff; address confirmed findings, re-run checks.
2. Commit, push branch `vk/616b-first-class-mcp`, open PR against the base branch.
