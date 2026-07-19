# Contracts: VK MCP Management UX

This document defines every interface, prop, and type change required by the redesign. No generated files (`shared/types.ts`) are modified.

---

## 1. McpServerDialog — Props and Result Changes

### Before

```typescript
// packages/web-core/src/shared/dialogs/settings/settings/McpServerDialog.tsx

export interface McpServerDialogProps {
  codec: McpServerCodec;
  existingNames: string[];
  initial?: { name: string; entry: JsonValue };
}

export type McpServerDialogResult =
  | { name: string; entry: JsonValue }
  | undefined;
```

### After

```typescript
export interface McpServerDialogProps {
  codec: McpServerCodec;            // retained: used to parse/serialize form fields
  existingNames: string[];
  profiles: SharedMcpProfile[];     // NEW: full list of MCP-capable profiles
  initial?: {
    name: string;
    entry: JsonValue;
    assignments: BaseCodingAgent[]; // NEW: pre-seeded for Edit; empty for Add
  };
}

export type McpServerDialogResult =
  | { name: string; entry: JsonValue; assignments: BaseCodingAgent[] }
  | undefined;
```

### Rationale
- `codec` is still the CLAUDE_CODE codec; it drives the server-definition form (parse/serialize). The codec prop is not per-profile — one shared form, multiple assignment targets.
- `profiles` enables the dialog to call `codecForAgent(profile.executor).transports` for each executor to derive compatibility dynamically as the transport changes.
- `assignments` moves from post-dialog logic in `openDialog` into the dialog's local state so it commits only on submit (Principle X).

---

## 2. McpServerDialog — New Internal State

The NiceModal re-seed `useEffect` (currently lines 145–154) must include the new `assignments` state:

```typescript
// New local state (initial render; the useEffect below re-seeds on every open)
const [assignments, setAssignments] = useState<BaseCodingAgent[]>(
  initial?.assignments ?? []
);

// Re-seed on modal.visible (extend existing useEffect)
useEffect(() => {
  if (!modal.visible) return;
  const resetForm = initialForm ?? emptyForm(codec.transports[0]);
  setName(initial?.name ?? '');
  setForm(resetForm);
  setArgsText((initialForm?.args ?? []).join('\n'));
  setCustomJson(isCustom ? JSON.stringify(initial!.entry, null, 2) : '');
  // For Edit, restore saved assignments.
  // For Add, seed ONE compatible default (the first profile that supports the
  // current transport) — not every compatible profile, and not an empty list.
  setAssignments(
    initial?.assignments ??
    profiles
      .filter(p => isTransportCompatible(p.executor, resetForm.transport))
      .slice(0, 1)
      .map(p => p.executor)
  );  // NEW
  setError(null);
}, [modal.visible, codec, initial]);
```

---

## 3. Compatibility Helper (new pure function, dialog-local)

```typescript
// To be defined at module scope inside McpServerDialog.tsx
// (or extracted to mcpServerCodec.ts if needed by tests)

/**
 * Returns null when executor is compatible with transport, or a human-readable
 * reason string when it is not. For 'unknown' transport (custom JSON), always
 * returns null (all agents selectable).
 */
function incompatibilityReason(
  executor: BaseCodingAgent,
  transport: McpTransport | null   // null = custom/unknown
): string | null {
  if (transport === null) return null;
  const supported = codecForAgent(executor).transports;
  if (supported.includes(transport)) return null;
  if (transport === 'sse') return t('settings.mcp.dialog.incompatible.noSse');
  if (transport === 'http') return t('settings.mcp.dialog.incompatible.noHttp');
  return t('settings.mcp.dialog.incompatible.generic', { transport });
}
```

Because this function uses `t()` from `useTranslation`, it should be implemented as an inline helper inside the component body (or as a pure function that accepts the `t` function).

**Usage in dialog**: called for every `profiles` entry on each render. `transport` is derived from the current form state using `transportOf(codec, codec.serialize({ ...form, args: argsFromLines(argsText) }))` — but since we always use the CLAUDE_CODE codec to serialize, and we have the `form.transport` value directly, the simplest approach is:

```typescript
// Derive current transport for compatibility checks:
// - form-based entry: form.transport (one of 'stdio', 'http', 'sse')
// - custom JSON: null (unknown, all agents compatible)
const currentTransport: McpTransport | null = isCustom ? null : form.transport;
```

---

## 4. Assignment Validation Rule

```typescript
// Inside validate() in McpServerDialog, before the existing name/entry checks:
if (assignments.length === 0) {
  setError(t('settings.mcp.validation.assignmentRequired'));
  return null;
}
```

Note: validation runs after name and entry checks (existing order preserved) — add the assignment check at the **end** of `validate()`, just before the final `return { name, entry }`.

---

## 5. McpSettingsSection — openDialog Simplification

### Before (lines 384–425)

```typescript
const openDialog = useCallback(async (server?: SharedMcpDraftServer) => {
  const codec = codecForAgent(BaseCodingAgentValue.CLAUDE_CODE);
  const result = await McpServerDialog.show({
    codec,
    existingNames: draft.servers.map(s => s.name).filter(n => n !== server?.name),
    initial: server ? { name: server.name, entry: entryForDialog(server.definition) } : undefined,
  });
  if (!result) return;
  const definition = definitionFromEntry(result.entry);
  const assignments = server?.assignments.length
    ? server.assignments
    : profiles.filter(...).slice(0, 1).map(p => p.executor);
  if (server && server.name !== result.name) {
    setDraft(prev => ({ ...prev, servers: prev.servers.filter(s => s.name !== server.name) }));
  }
  setServer({ name: result.name, definition, assignments });
}, [draft.servers, profiles, setServer]);
```

### After

```typescript
const openDialog = useCallback(async (server?: SharedMcpDraftServer) => {
  const codec = codecForAgent(BaseCodingAgentValue.CLAUDE_CODE);
  const result = await McpServerDialog.show({
    codec,
    profiles,
    existingNames: draft.servers.map(s => s.name).filter(n => n !== server?.name),
    initial: server
      ? { name: server.name, entry: entryForDialog(server.definition), assignments: server.assignments }
      : undefined,
  });
  if (!result) return;
  if (server && server.name !== result.name) {
    setDraft(prev => ({ ...prev, servers: prev.servers.filter(s => s.name !== server.name) }));
  }
  setServer({ name: result.name, definition: definitionFromEntry(result.entry), assignments: result.assignments });
}, [draft.servers, profiles, setServer]);
```

Changes: `profiles` added to call site; `initial` gains `assignments`; post-dialog assignment logic removed; `result.assignments` used directly.

---

## 6. McpSettingsSection — Removed Callbacks

| Callback | Lines | Action |
|----------|-------|--------|
| `toggleAssignment` | 448–467 | **Delete** — inline mutation, replaced by modal |

---

## 7. McpSettingsSection — Server Card Layout

The inline assignment grid (lines 1022–1069) is **replaced** with a compact assignment summary badge. The rest of the card (name, transport badge, auth badge, Test/Edit/Delete buttons, `TestResultDetails`, secondary error list) is **preserved unchanged**.

```typescript
// Replace lines 1022-1069 with:
{server.assignments.length === 0 ? (
  <span className="text-xs text-low italic">
    {t('settings.mcp.labels.noAssignments')}
  </span>
) : (
  <div className="mt-1 flex flex-wrap gap-1">
    {server.assignments.map(executor => (
      <span
        key={executor}
        className="rounded-sm bg-primary border border-border px-1.5 py-0.5 text-xs text-low"
      >
        {toPrettyCase(executor)}
      </span>
    ))}
  </div>
)}
```

The status icon for each assigned executor (currently only shown in the checkbox grid) moves onto each badge:

```typescript
{server.assignments.map(executor => {
  const result = testResults[testKey(server.name, executor)]?.result;
  return (
    <span key={executor} className="inline-flex items-center gap-1 rounded-sm bg-primary border border-border px-1.5 py-0.5 text-xs text-low">
      {toPrettyCase(executor)}
      <McpTestStatusIcon result={result} />
    </span>
  );
})}
```

---

## 8. New i18n Keys Required

Keys to add to `settings.mcp` in all 7 locale files:

```json
{
  "settings": {
    "mcp": {
      "dialog": {
        "agentsTitle": "Assign to agents",
        "agentsHelper": "Choose which coding agents should use this MCP server.",
        "incompatible": {
          "noSse": "This agent does not support SSE transport.",
          "noHttp": "This agent only supports stdio (command) transport.",
          "generic": "This agent does not support {{transport}} transport."
        }
      },
      "validation": {
        "assignmentRequired": "At least one agent must be assigned."
      },
      "labels": {
        "noAssignments": "No agents assigned"
      }
    }
  }
}
```

Keys to **update** (copy change, not structural):

| Key | Current | New |
|-----|---------|-----|
| `settings.mcp.labels.assignmentsHelper` | "Each server is written to the native config file for every selected compatible agent." | Keep as-is — it remains accurate |
| `settings.mcp.dialog.description` | "Configure how this MCP server is launched." | "Configure this MCP server and assign it to agents." |

Non-English locales should receive matching translations or safe neutral fallbacks (e.g. copy the English string) — no locale file should be left with a missing key.

---

## 9. Preserved Contracts (no changes)

| Contract | Preserved? |
|----------|-----------|
| `SharedMcpDraftServer` type | Yes — unchanged |
| `SharedMcpDraftState` type | Yes — unchanged |
| `sharedMcpSnapshot()` | Yes — unchanged |
| `draftFromSharedRead()` | Yes — unchanged |
| `inputsFromDraft()` | Yes — unchanged |
| `testKey(serverName, executor)` | Yes — unchanged |
| `testResults` state shape | Yes — `Record<string, SharedMcpAssignmentTestResult>` |
| `mergeOAuthRefresh()` | Yes — unchanged |
| `SettingsSaveBar` props | Yes — unchanged |
| JSON mode / `applyJson` / `enterJsonMode` | Yes — unchanged |
| Conflict resolution UI | Yes — unchanged |
| `connectAssignment` / `finalizeConnected` / OAuth flow | Yes — all unchanged |
| `McpServerCodec` interface | Yes — unchanged |
| `codecForAgent()` | Yes — unchanged |
