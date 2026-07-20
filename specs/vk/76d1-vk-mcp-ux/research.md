# Research Notes: VK MCP Management UX

## Files Examined

| File | Lines | Purpose |
|------|-------|---------|
| `packages/web-core/src/shared/dialogs/settings/settings/McpSettingsSection.tsx` | 1087 | Primary settings surface — owns all state |
| `packages/web-core/src/shared/dialogs/settings/settings/McpServerDialog.tsx` | 441 | Add/edit modal |
| `packages/web-core/src/shared/lib/sharedMcpSettingsState.ts` | 238 | Draft model, snapshot, helpers |
| `packages/web-core/src/shared/lib/mcpServerCodec.ts` | 457 | Per-agent codecs, transport parse/serialize |
| `packages/web-core/src/shared/lib/sharedMcpSettingsState.test.ts` | 301 | Existing state tests |
| `packages/web-core/src/shared/lib/mcpServerCodec.test.ts` | 317 | Existing codec tests |
| `packages/web-core/src/i18n/locales/en/settings.json` | 793 | English i18n strings |
| `shared/types.ts` (excerpts) | — | Generated TS types |

---

## Key Type Inventory (from shared/types.ts)

```typescript
// Generated — do not edit
enum BaseCodingAgent {
  CLAUDE_CODE = "CLAUDE_CODE", AMP = "AMP", GEMINI = "GEMINI",
  CODEX = "CODEX", OPENCODE = "OPENCODE", CURSOR_AGENT = "CURSOR_AGENT",
  QWEN_CODE = "QWEN_CODE", COPILOT = "COPILOT", DROID = "DROID", GROK = "GROK"
}

type SharedMcpProfile = {
  executor: BaseCodingAgent;
  display_name: string;
  supports_mcp: boolean;
  config_path: string | null;
  servers_path: string[];
  read_error: string | null;
};

type McpServerDefinition = {
  transport: "stdio" | "http" | "sse" | "unknown";
  value: JsonValue;
  representable_in_form: boolean;
};

type SharedMcpAssignment = {
  executor: BaseCodingAgent;
  native_name: string;
  native_entry: JsonValue | null;
  has_credentials: boolean;
  representable: boolean;
  incompatibility_reason: string | null;
};

type SharedMcpCompatibility = {
  executor: BaseCodingAgent;
  compatible: boolean;
  reason: string | null;
};

type SharedMcpServer = {
  name: string;
  definition: McpServerDefinition;
  assignments: SharedMcpAssignment[];
  source_kind: SharedMcpSourceKind;
  native_sources: NativeMcpSource[];
  compatibility: SharedMcpCompatibility[];   // backend-computed, stale by definition
  auth_mode: SharedMcpAuthMode;
  gateway_status: string | null;
};

// Local draft (mutable) — not a generated type
type SharedMcpDraftServer = {    // defined in sharedMcpSettingsState.ts
  name: string;
  definition: McpServerDefinition;
  assignments: BaseCodingAgent[];
};

type SharedMcpDraftState = {
  servers: SharedMcpDraftServer[];
  conflicts: SharedMcpConflict[];
};
```

---

## McpSettingsSection.tsx — State Inventory

| State | Type | Purpose |
|-------|------|---------|
| `readModel` | `SharedMcpReadResponse \| null` | Last server-read; used by save/discard/OAuth refresh |
| `draft` | `SharedMcpDraftState` | Editable local copy; drives all UI |
| `originalSnapshot` | `string` | JSON of initial draft for dirty detection |
| `testResults` | `Record<string, SharedMcpAssignmentTestResult>` | Keyed by `testKey(serverName, executor)` |
| `connectingKey`, `connectErrors`, `loopbackEnabled`, `manualFlow`, `manualCode`, `completing` | various | OAuth connect flow state |

Key derived values:
- `profiles` = `readModel?.profiles.filter(p => p.supports_mcp) ?? []` (line 300)
- `serverByName` = `Map<string, SharedMcpServer>` from `readModel?.servers` (line 301)
- `isDirty` = `snapshot !== originalSnapshot` (line 266)

---

## McpServerDialog.tsx — Current Contract

```typescript
// Current props
interface McpServerDialogProps {
  codec: McpServerCodec;          // always codecForAgent(CLAUDE_CODE) in current caller
  existingNames: string[];
  initial?: { name: string; entry: JsonValue };
}

// Current result type
type McpServerDialogResult = { name: string; entry: JsonValue } | undefined;
```

The dialog is created via `NiceModal` (`create` + `defineModal`). The component is **reused across opens** — state initializers don't re-run. The `useEffect` on `modal.visible` re-seeds all local state (lines 145–154). Any new state added for assignments must be included in that same effect.

### Dialog internal state (lines 130–140):
```typescript
const [name, setName] = useState(initial?.name ?? '');
const [form, setForm] = useState<McpServerFormValues>(...);
const [argsText, setArgsText] = useState(...);
const [customJson, setCustomJson] = useState(...);
const [error, setError] = useState<string | null>(null);
```

### Validation flow (lines 159–237):
The `validate()` function returns `{ name, entry } | null`. On success, `handleSave` resolves the modal with the result and hides it. On cancel, it resolves `undefined`.

---

## Current Assignment UI (to be removed)

**Lines 1022–1069 of McpSettingsSection.tsx** render an inline checkbox grid per server card:
- For each `profiles` entry, renders a `<div>` with a `<label>` + `<input type="checkbox">`.
- `checked={server.assignments.includes(profile.executor)}`
- `disabled={incompatible}` where `incompatible = compatibility?.compatible === false` — this reads **backend** `SharedMcpCompatibility[]`, which is computed from the *saved* state and can be stale.
- `onChange={() => toggleAssignment(server.name, profile)}` — **mutates draft directly** (violates Principle X).

The `toggleAssignment` callback (lines 448–467) updates `draft` inline without any transactional boundary.

---

## Compatibility Logic (mcpServerCodec.ts)

`codecForAgent(executor: BaseCodingAgent): McpServerCodec` — lines 428–448.

| Agent | Codec | Transports |
|-------|-------|-----------|
| CODEX | `codexCodec` | Editor codec exposes `['stdio']`; shared assignment compatibility also supports streamable `http` |
| OPENCODE | `opencodeCodec` | `['stdio', 'http']` |
| GEMINI, QWEN_CODE | `geminiCodec` | `['stdio', 'http']` |
| CURSOR_AGENT, GROK | `cursorCodec` | `['stdio', 'http']` |
| CLAUDE_CODE, AMP, COPILOT, DROID | `claudeStyleCodec` | `['stdio', 'http', 'sse']` |

**Transport compatibility rule** (derived from codec.transports):
- If transport is `'sse'` and executor is CODEX, OPENCODE, GEMINI, QWEN_CODE, CURSOR_AGENT, or GROK → incompatible.
- If transport is `'http'` and executor is CODEX → incompatible.
- If transport is `'unknown'` (custom JSON) → no filtering; all agents are assignable.
- All other combinations → compatible.

Implementation: `codecForAgent(executor).transports.includes(transport)` is the correct runtime check. No need to call the backend's `SharedMcpCompatibility[]`.

---

## openDialog Callback — Current Post-Dialog Assignment Logic (lines 384–425)

```typescript
const openDialog = useCallback(async (server?: SharedMcpDraftServer) => {
  const codec = codecForAgent(BaseCodingAgentValue.CLAUDE_CODE);
  const result = await McpServerDialog.show({ codec, existingNames, initial });
  if (!result) return;

  const definition = definitionFromEntry(result.entry);
  // Assignment logic OUTSIDE dialog — wrong place, will move inside:
  const assignments = server?.assignments.length
    ? server.assignments                        // use existing if editing
    : profiles
        .filter(profile => !(
          (profile.executor === CODEX && definition.transport !== 'stdio') ||
          (profile.executor === GROK  && definition.transport === 'sse')
        ))
        .slice(0, 1)
        .map(p => p.executor);                 // default: first compatible profile

  // rename: remove old entry, add new
  if (server && server.name !== result.name) {
    setDraft(prev => ({ ...prev, servers: prev.servers.filter(s => s.name !== server.name) }));
  }
  setServer({ name: result.name, definition, assignments });
}, [...]);
```

After the redesign, this callback becomes much simpler: all assignment logic moves into the dialog, and the callback just writes the returned `{ name, entry, assignments }` to the draft.

---

## i18n — Existing Keys Under `settings.mcp`

Currently present (English, `packages/web-core/src/i18n/locales/en/settings.json`):

```
settings.mcp.title
settings.mcp.description
settings.mcp.loading
settings.mcp.labels.servers
settings.mcp.labels.assignments
settings.mcp.labels.assignmentsHelper
settings.mcp.list.empty
settings.mcp.list.addServer
settings.mcp.delete
settings.mcp.deleteConfirm
settings.mcp.auth.sharedGateway / explicitHeader / agentNative / none
settings.mcp.auth.reconnect / disconnect / disconnectConfirm
settings.mcp.auth.cloudflareClientId / cloudflareClientSecret
settings.mcp.dialog.addTitle / editTitle / description / name / namePlaceholder
settings.mcp.dialog.transport / command / args / argsHelper / env / addEnv
settings.mcp.dialog.url / headers / addHeader / customJson / customJsonHelper
settings.mcp.dialog.cancel / add / saveEdit
settings.mcp.validation.*  (7 keys)
settings.mcp.json.editAsJson / editAsForm
settings.mcp.errors.*  (7 keys)
settings.mcp.save.*  (5 keys)
settings.mcp.test.*  (13 keys)
settings.mcp.conflicts.*  (4 keys)
```

---

## Locale Files to Update (7 files)

```
packages/web-core/src/i18n/locales/en/settings.json   ← primary (full translations)
packages/web-core/src/i18n/locales/fr/settings.json
packages/web-core/src/i18n/locales/es/settings.json
packages/web-core/src/i18n/locales/ja/settings.json
packages/web-core/src/i18n/locales/ko/settings.json
packages/web-core/src/i18n/locales/zh-Hant/settings.json
packages/web-core/src/i18n/locales/zh-Hans/settings.json
```

---

## Test Files

Existing tests (Vitest):
- `mcpServerCodec.test.ts` — 22 test cases, pure functions, all must remain green.
- `sharedMcpSettingsState.test.ts` — 8 test cases, pure functions, all must remain green.

No DOM component tests exist for `McpSettingsSection` or `McpServerDialog`. New tests will be pure-function or light component tests per constitution Principle II.

---

## Remote-Web Scope Check

Code search confirms: no MCP settings components in `packages/remote-web/`. Remote-web is out of scope.

---

## Invariants Confirmed

1. `draft` is the single source of truth for UI rendering; `readModel` is used only by save, discard, and OAuth merge.
2. `testResults` shape `Record<string, SharedMcpAssignmentTestResult>` does not change; it's already keyed by assignment.
3. `mergeOAuthRefresh`, `finalizeConnected`, `connectAssignment`, `completeManualAuth` — all OAuth helpers operate on `serverName + executor` and are not affected by moving assignment editing into the modal.
4. The `SettingsSaveBar` shows when `isDirty`; its props don't change.
5. `inputsFromDraft` filters out `transport === 'unknown'` servers before writing — this is correct and unchanged.
6. JSON mode (`jsonMode`) round-trips `SharedMcpDraftServer[]` via `inputsFromDraft`/`applyJson` — this is not changed.
7. Conflict resolution (`resolveConflictVariant`) already applies transport compatibility filtering to assignments — unchanged.
