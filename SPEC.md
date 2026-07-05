# Spec: First-Class MCP Server Configuration UX

## Problem

The MCP Servers settings section (`packages/web-core/src/shared/dialogs/settings/settings/McpSettingsSection.tsx`) exposes a raw JSON textarea containing the *entire* agent config wrapper (e.g. `{"mcpServers": {...}}`). Users must hand-write JSON to add, edit, or remove an MCP server, know each agent's native config dialect (Claude-style `mcpServers`, Gemini's `httpUrl`, Opencode's `type: local/remote` + command array, Codex's TOML-backed `mcp_servers`), and get only a coarse "Invalid JSON format" error when they make a mistake. On mobile/tablet (see attached screenshot) editing JSON in a textarea is especially painful.

## Goal

Replace JSON-first editing with a structured, form-based UX:

- A **server list** showing each configured server as a card with its name, transport badge, and a one-line summary (command line or URL), plus edit/remove actions.
- An **add/edit dialog** with proper fields — name, transport type, command/args/env for stdio servers, URL/headers for remote servers — with inline validation (no hand-written JSON for the common paths).
- **Popular servers** cards add directly to the list with one click (same behavior as today, but no JSON round-trip visible to the user).
- A **raw JSON escape hatch** ("Edit as JSON" toggle) that preserves today's full-fidelity editing for power users and for config shapes the form can't represent.

## Non-goals

- No backend / API changes. The existing `GET/POST /mcp-config?executor=...` endpoints already exchange the servers map in agent-native form; the frontend keeps using them via `machineClient.loadMcpServers` / `saveMcpServers`.
- No change to generated types (`shared/types.ts`) — `McpConfig` already carries `servers`, `servers_path`, `template`, `preconfigured`.
- No change to how launched agents receive MCP config (`crates/executors/src/mcp_config.rs` adapters).
- No secret management — env var / header values remain plain text, same as the JSON editor today.

## Background: per-agent server entry formats

The API returns/accepts server entries in the *agent's native* format (the canonical→native conversion in `crates/executors/src/mcp_config.rs` applies only to preconfigured servers). The form layer therefore needs per-agent codecs:

| Agent(s) | stdio shape | remote shape |
|---|---|---|
| Claude Code, Amp, Droid | `{command, args?, env?}` | `{type: "http"\|"sse", url, headers?}` |
| Copilot | `{command, args?, env?, tools?}` | `{type: "http"\|"sse", url, headers?, tools?}` |
| Cursor | `{command, args?, env?}` | `{url, headers?}` (no `type`) |
| Gemini, Qwen Code | `{command, args?, env?}` | `{httpUrl, headers?}` |
| Codex (TOML) | `{command, args?, env?}` | *not supported* — stdio only |
| Opencode | `{type: "local", command: [cmd, ...args], enabled, environment?}` | `{type: "remote", url, headers?, enabled}` |

## Design

### 1. Normalized form model + per-agent codecs

New module `packages/web-core/src/shared/lib/mcpServerCodec.ts`:

```ts
export type McpTransport = 'stdio' | 'http' | 'sse';

export interface McpServerFormValues {
  transport: McpTransport;
  command: string;
  args: string[];
  env: Array<{ key: string; value: string }>;
  url: string;
  headers: Array<{ key: string; value: string }>;
}

export interface McpServerCodec {
  transports: McpTransport[];
  parse(entry: JsonValue): McpServerFormValues | null; // null → custom
  serialize(values: McpServerFormValues, original?: JsonValue): JsonValue;
  summarize(entry: JsonValue): string;
}

export function codecForAgent(agent: BaseCodingAgent): McpServerCodec;
```

Codec rules:

- **Parse is conservative.** An entry parses only if its recognized keys have the expected types. Anything else returns `null` and is treated as a **custom** entry (edited as raw JSON).
- **Unknown keys are preserved.** Extra keys the form doesn't model (Copilot's `tools`, Opencode's `enabled`, `timeout`, etc.) never block parsing; on save they merge back from the original entry unchanged. `parse` → `serialize(values, original)` on an untouched form is a no-op for every representable entry.
- **Per-agent specifics:**
  - Claude/Amp/Droid/Copilot: `type` absent + `command` present ⇒ stdio; `type: "http"|"sse"` + `url` ⇒ remote. Both `http` and `sse` offered.
  - Cursor: remote entries have `url` and no `type`; serialize omits `type`. Single URL transport option (rendered as `http`).
  - Gemini/Qwen: remote uses `httpUrl`. Single URL transport option.
  - Codex: `transports = ['stdio']` only.
  - Opencode: `type: "local"` ⇒ stdio with `command[0]` as command, rest as args, `environment` as env; `type: "remote"` ⇒ url/headers. New entries serialize with `enabled: true`; existing `enabled` value is preserved.

### 2. Section state: object-based, not string-based

`McpSettingsSection` keeps the servers map (`Record<string, JsonValue>`) as source of truth. Dirty tracking compares `JSON.stringify(servers)` against a snapshot taken at load/save. Save posts `{servers}` via `machineClient.saveMcpServers`. Save-bar behavior unchanged.

### 3. Server list UI

One card per entry: server name (mono), transport badge (`stdio` / `HTTP` / `SSE` / `custom`), summary line (`codec.summarize`), Edit and Remove buttons. Empty state + "Add server" button. Remove deletes the key locally, applied on Save (discardable).

### 4. Add/edit dialog

`McpServerDialog` (NiceModal), receives the codec, existing names, and (for edit) name + original entry:

- **Name**: required, unique, no leading/trailing whitespace. Rename allowed (delete old key, insert new).
- **Transport**: select from `codec.transports`; hidden when only one option.
- **stdio**: Command (required); Arguments (textarea, one per line); Environment key/value rows.
- **http/sse**: URL (required, http(s) URL); Headers key/value rows.
- **Custom entries** (`parse` → `null`): single JSON textarea for that entry, validated as a JSON object.

### 5. Popular servers

`addPreconfigured(key)` writes `preconfigured[key]` (already agent-native) into `servers`. Cards for already-added servers show a check state and are disabled.

### 6. Raw JSON escape hatch

"Edit as JSON" toggle. JSON mode seeds the textarea with `createFullConfig`-shaped JSON built from current `servers`; edits re-validate/extract live. Switching back to form mode is blocked on invalid JSON. For Codex the textarea stays JSON (backend converts to TOML).

### 7. i18n

New keys under `settings.mcp.*` in `en/settings.json` (list, dialog, validation, json-toggle). i18next falls back to English for other locales.

## Validation rules (summary)

| Field | Rule |
|---|---|
| Name | non-empty; unique; no leading/trailing whitespace |
| Command | non-empty (stdio) |
| Args | lines used verbatim; empty lines dropped |
| URL | non-empty; parses via `new URL()` with http/https scheme |
| Env/Header keys | non-empty; unique within the entry |
| Custom JSON | parses as a JSON object |

## Testing

- Vitest unit tests `mcpServerCodec.test.ts`: parse/serialize round-trips per agent (unknown-key preservation, Opencode command split/join, Gemini `httpUrl`, Cursor typeless URL, Codex stdio-only), unparseable fallbacks, summarize output.
- `pnpm run check` and `pnpm run lint` pass.
- Manual: load per agent, add popular server, add stdio + http via form, edit, remove, JSON toggle round-trip, save & confirm file on disk.

## Acceptance criteria

1. Add / edit / remove MCP servers for every MCP-capable agent without seeing or typing JSON.
2. Existing configs — including entries with keys the form doesn't model — survive load → edit-something-else → save without data loss.
3. Popular-server cards add in one click.
4. Raw JSON editing remains available and behaves as before.
5. Per-agent constraints enforced (Codex: stdio only; Gemini/Qwen/Cursor: single URL transport).
6. No backend or generated-type changes; web `check`/`lint`/`test` green.
