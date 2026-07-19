# Tasks: VK MCP Management UX

Derived from `specs/vk/76d1-vk-mcp-ux/plan.md`, `spec.md`, and `contracts.md`.
Ordered by dependency wave; tasks within the same wave may run in parallel.

---

## Wave 0 — Baseline (must be green before any changes)

### [x] T0 · Establish green baseline

**Depends on**: nothing
**Blocks**: everything

Run both checks from the repo root and confirm they pass. If either is red, fix
it before touching feature files.

```sh
pnpm run check
pnpm run lint
```

**Done when**: both commands exit 0 with no errors in the affected files.

---

## Wave 1 — Leaf changes (parallel)

These two tasks are independent of each other and can land in any order.

---

### [x] T1 · Export `isTransportCompatible` from `mcpServerCodec.ts`

**Depends on**: T0
**Blocks**: T3, T4

**File**: `packages/web-core/src/shared/lib/mcpServerCodec.ts`

Add a new exported pure function after the existing `codecForAgent` export:

```typescript
export function isTransportCompatible(
  executor: BaseCodingAgent,
  transport: McpTransport
): boolean {
  return codecForAgent(executor).transports.includes(transport);
}
```

No other changes to this file.

**Verification**:
```sh
pnpm run check          # TypeScript compiles
grep -n 'isTransportCompatible' packages/web-core/src/shared/lib/mcpServerCodec.ts
```

---

### [x] T2 · Add i18n keys to all 7 locale files

**Depends on**: T0
**Blocks**: T4

**Files** (all 7 must be updated):
```
packages/web-core/src/i18n/locales/en/settings.json
packages/web-core/src/i18n/locales/fr/settings.json
packages/web-core/src/i18n/locales/es/settings.json
packages/web-core/src/i18n/locales/ja/settings.json
packages/web-core/src/i18n/locales/ko/settings.json
packages/web-core/src/i18n/locales/zh-Hant/settings.json
packages/web-core/src/i18n/locales/zh-Hans/settings.json
```

#### 2a · English locale — `en/settings.json`

Under `settings.mcp.dialog`, add (alongside existing keys):
```json
"agentsTitle": "Assign to agents",
"agentsHelper": "Choose which coding agents should use this MCP server.",
"incompatible": {
  "noSse": "This agent does not support SSE transport.",
  "noHttp": "This agent only supports stdio (command) transport.",
  "generic": "This agent does not support {{transport}} transport."
}
```

Update existing key:
```json
"description": "Configure this MCP server and assign it to agents."
```

Under `settings.mcp.validation`, add:
```json
"assignmentRequired": "At least one agent must be assigned."
```

Under `settings.mcp.labels`, add:
```json
"noAssignments": "No agents assigned"
```

#### 2b · Non-English locales (fr, es, ja, ko, zh-Hant, zh-Hans)

Add the identical key structure to each file. Use the English strings verbatim as
safe fallbacks for any locale without a dedicated translation — no key may be
absent at runtime.

**Verification**:
```sh
# Confirm all 7 files contain the new keys
for f in packages/web-core/src/i18n/locales/*/settings.json; do
  echo "=== $f ===" && grep -c 'assignmentRequired\|agentsTitle\|noAssignments' "$f"
done
# Each file should report 3
pnpm run check
```

---

## Wave 2 — Core changes (parallel after T1 + T2)

These two tasks both unblock at the same time; T3 and T4 can run in parallel
since T3 reads a different file from T4.

---

### [x] T3 · Add `isTransportCompatible` tests

**Depends on**: T1
**Blocks**: T6

**File**: `packages/web-core/src/shared/lib/mcpServerCodec.test.ts`
(Create file if absent; check first with `ls packages/web-core/src/shared/lib/`)

Add a `describe` block that imports `isTransportCompatible` and `BaseCodingAgent`
from the same module, then asserts:

```typescript
import { isTransportCompatible } from './mcpServerCodec';
import { BaseCodingAgent } from 'shared/types';

describe('isTransportCompatible', () => {
  it('allows CODEX streamable HTTP assignments', () => {
    expect(isTransportCompatible(BaseCodingAgent.CODEX, 'http')).toBe(true);
  });
  it('reports compatibility for CLAUDE_CODE with sse', () => {
    expect(isTransportCompatible(BaseCodingAgent.CLAUDE_CODE, 'sse')).toBe(true);
  });
  it('reports incompatibility for GROK with sse', () => {
    expect(isTransportCompatible(BaseCodingAgent.GROK, 'sse')).toBe(false);
  });
});
```

Confirm the executor/transport values above match what `codecForAgent` actually
returns — read `mcpServerCodec.ts` before writing so the test values are real.

**Verification**:
```sh
pnpm exec vitest run packages/web-core/src/shared/lib/mcpServerCodec.test.ts
```

---

### [x] T4 · Update `McpServerDialog.tsx` — props, state, validation, UI

**Depends on**: T1 (uses `isTransportCompatible`), T2 (uses new i18n keys)
**Blocks**: T5

**File**: `packages/web-core/src/shared/dialogs/settings/settings/McpServerDialog.tsx`

Read the entire file before making any change. Apply the following sub-steps in
order (they are sequential within this file):

#### 4a · Update imports

Add only what is missing:
```typescript
import type { BaseCodingAgent, JsonValue, SharedMcpProfile } from 'shared/types';
import { isTransportCompatible } from '@/shared/lib/mcpServerCodec';
import { toPrettyCase } from '@/shared/lib/string';
```

#### 4b · Replace `McpServerDialogProps` and `McpServerDialogResult` (contracts.md §1)

```typescript
export interface McpServerDialogProps {
  codec: McpServerCodec;
  existingNames: string[];
  profiles: SharedMcpProfile[];
  initial?: {
    name: string;
    entry: JsonValue;
    assignments: BaseCodingAgent[];
  };
}

export type McpServerDialogResult =
  | { name: string; entry: JsonValue; assignments: BaseCodingAgent[] }
  | undefined;
```

#### 4c · Add `assignments` local state

Inside `McpServerDialogImpl`, add alongside existing `useState` declarations:
```typescript
const [assignments, setAssignments] = useState<BaseCodingAgent[]>(
  initial?.assignments ?? []
);
```

#### 4d · Extend the NiceModal re-seed `useEffect` (contracts.md §2)

Find the existing `useEffect` that fires on `modal.visible` and re-seeds
`name`, `form`, `argsText`, `customJson`, `error`. Capture the reset form in a
local variable first, then replace the existing `setForm(...)` call and add the
assignment reset to the same effect body:

```typescript
const resetForm = initialForm ?? emptyForm(codec.transports[0]);
setForm(resetForm);
// … other existing setters …
// For Edit, restore saved assignments.
// For Add, seed ONE compatible default — the first profile that supports the
// current transport. Do not pre-select every compatible profile.
setAssignments(
  initial?.assignments ??
  profiles
    .filter(p => isTransportCompatible(p.executor, resetForm.transport))
    .slice(0, 1)
    .map(p => p.executor)
);
```

This is the **single sensible compatible default for Add** (FR-5): when no
`initial` is present (new server), exactly one compatible profile is
pre-selected — not zero (which would force immediate validation friction) and
not all compatible profiles. `resetForm.transport` is used instead of
`form.transport` so the filter sees the freshly reset value, not stale state.

#### 4e · Derive `currentTransport` for compatibility (contracts.md §3)

Below the existing `isCustom` constant:
```typescript
const currentTransport: McpTransport | null = isCustom ? null : form.transport;
```

#### 4f · Add `getIncompatibilityReason` inline helper (contracts.md §3)

Inside the component body, after `const { t } = useTranslation('settings')`:
```typescript
const getIncompatibilityReason = (executor: BaseCodingAgent): string | null => {
  if (currentTransport === null) return null;
  if (isTransportCompatible(executor, currentTransport)) return null;
  if (currentTransport === 'sse') return t('settings.mcp.dialog.incompatible.noSse');
  if (currentTransport === 'http') return t('settings.mcp.dialog.incompatible.noHttp');
  return t('settings.mcp.dialog.incompatible.generic', { transport: currentTransport });
};
```

#### 4g · Add assignment validation to `validate()` (contracts.md §4)

At the **end** of `validate()`, just before the final `return`:
```typescript
if (assignments.length === 0) {
  setError(t('settings.mcp.validation.assignmentRequired'));
  return null;
}
```

#### 4h · Change `validate()` return to include `assignments`

```typescript
// Before:
return { name: trimmedName, entry };
// After:
return { name: trimmedName, entry, assignments };
```

Confirm `defineModal<McpServerDialogProps, McpServerDialogResult>` at the bottom
of the file reflects the updated result type.

#### 4i · Add agent assignment UI section (plan.md §1i)

After the transport/command/URL fields and **above** the `{error && ...}` Alert,
insert:

```tsx
{profiles.length > 0 && (
  <div className="space-y-2">
    <Label>{t('settings.mcp.dialog.agentsTitle')}</Label>
    <p className="text-xs text-low">{t('settings.mcp.dialog.agentsHelper')}</p>
    <div className="space-y-1">
      {profiles.map((profile) => {
        const reason = getIncompatibilityReason(profile.executor);
        const incompatible = reason !== null;
        const checked = assignments.includes(profile.executor);
        return (
          <label
            key={profile.executor}
            className={cn(
              'flex items-start gap-2 rounded-sm px-2 py-1.5 text-sm',
              incompatible ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'
            )}
          >
            <input
              type="checkbox"
              checked={checked}
              disabled={incompatible}
              onChange={() => {
                setAssignments((prev) =>
                  checked
                    ? prev.filter((e) => e !== profile.executor)
                    : [...prev, profile.executor]
                );
                setError(null);
              }}
              className="mt-0.5 shrink-0"
            />
            <span className="min-w-0">
              <span className="font-medium">{toPrettyCase(profile.executor)}</span>
              {incompatible && (
                <span className="block text-xs text-error">{reason}</span>
              )}
            </span>
          </label>
        );
      })}
    </div>
  </div>
)}
```

**Verification**:
```sh
pnpm run check          # must pass
pnpm run lint           # must pass
```

---

## Wave 3 — Section update (after T4)

### [x] T5 · Update `McpSettingsSection.tsx` — openDialog, toggleAssignment, card badges

**Depends on**: T4
**Blocks**: T6

**File**: `packages/web-core/src/shared/dialogs/settings/settings/McpSettingsSection.tsx`

Read the entire file before making any change. Apply sub-steps in order:

#### 5a · Replace `openDialog` callback (contracts.md §5)

Find `openDialog` (currently around lines 384–425). Replace its body with the
simplified version from contracts.md §5:

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

Ensure `profiles` is in the `useCallback` dependency array. `profiles` is
already derived earlier in the file at:
```typescript
const profiles = readModel?.profiles.filter((profile) => profile.supports_mcp) ?? [];
```
No change to that derivation.

#### 5b · Delete `toggleAssignment` callback (contracts.md §6)

Find and delete the `toggleAssignment` callback (currently around lines 448–467).
Also remove any references to it in JSX (the inline assignment grid that called it
is removed in step 5c).

#### 5c · Replace inline assignment grid with compact badges (contracts.md §7)

Find the inline assignment grid block (currently around lines 1022–1069,
`<div className="mt-2 grid gap-1 ...">` containing profile checkboxes) and
replace it with:

```tsx
{server.assignments.length === 0 ? (
  <span className="text-xs text-low italic">
    {t('settings.mcp.labels.noAssignments')}
  </span>
) : (
  <div className="mt-1 flex flex-wrap gap-1">
    {server.assignments.map(executor => {
      const result = testResults[testKey(server.name, executor)]?.result;
      return (
        <span
          key={executor}
          className="inline-flex items-center gap-1 rounded-sm bg-primary border border-border px-1.5 py-0.5 text-xs text-low"
        >
          {toPrettyCase(executor)}
          <McpTestStatusIcon result={result} />
        </span>
      );
    })}
  </div>
)}
```

Preserve everything else in the card: server name + transport badge + auth badge,
Test/Edit/Delete button row, `TestResultDetails` block, secondary error list.

**Verification**:
```sh
pnpm run check          # must pass
pnpm run lint           # must pass
grep -n 'toggleAssignment' packages/web-core/src/shared/dialogs/settings/settings/McpSettingsSection.tsx
# must print nothing
grep -n 'mt-2 grid gap-1' packages/web-core/src/shared/dialogs/settings/settings/McpSettingsSection.tsx
# must print nothing
```

---

## Wave 4 — Final verification (after all tasks)

### [x] T6 · Format, type-check, lint, and test

**Depends on**: T3, T5
**Blocks**: nothing (ship gate)

Run in order:

```sh
# 1. Format — must produce no diff
pnpm run format
git diff --exit-code

# 2. Full type check
pnpm run check

# 3. Full lint
pnpm run lint

# 4. Targeted Vitest
pnpm exec vitest run packages/web-core/src/shared/lib/mcpServerCodec.test.ts

# 5. All existing tests still green
cargo test --workspace    # no Rust expected to be touched; confirm nothing regressed
```

**Visual verification** (when local env is available):
```sh
pnpm run dev
```
Open Settings → MCP Servers and confirm:
- [ ] Cards show agent badges (not checkboxes)
- [ ] Add dialog shows agent assignment section; one compatible agent pre-selected by default
- [ ] Selecting no agents and submitting shows validation error
- [ ] Canceling Edit leaves draft unchanged
- [ ] Renaming a server replaces it without duplicate
- [ ] OAuth Connect remains reachable when `auth_required`
- [ ] JSON mode round-trips `SharedMcpDraftServer[]` correctly
- [ ] Narrow layout (~480 px) is usable

**Done when**: all commands exit 0 and visual checks pass.

---

## Dependency Graph Summary

```
T0
├── T1 ─── T3 ──────────────────────┐
│                                    ├─ T6
└── T2 ─── T4 ─── T5 ───────────────┘
```

| Task | Depends on | Can parallelize with |
|------|-----------|---------------------|
| T0   | —          | —                   |
| T1   | T0         | T2                  |
| T2   | T0         | T1                  |
| T3   | T1         | T4                  |
| T4   | T1, T2     | T3                  |
| T5   | T4         | —                   |
| T6   | T3, T5     | —                   |

---

## Files Changed

| File | Task | Change |
|------|------|--------|
| `packages/web-core/src/shared/lib/mcpServerCodec.ts` | T1 | Export `isTransportCompatible` (~4 lines) |
| `packages/web-core/src/shared/lib/mcpServerCodec.test.ts` | T3 | Add 3 pure-function tests |
| `packages/web-core/src/i18n/locales/en/settings.json` | T2 | Add 6 keys, update 1 |
| `packages/web-core/src/i18n/locales/fr/settings.json` | T2 | Add same keys |
| `packages/web-core/src/i18n/locales/es/settings.json` | T2 | Add same keys |
| `packages/web-core/src/i18n/locales/ja/settings.json` | T2 | Add same keys |
| `packages/web-core/src/i18n/locales/ko/settings.json` | T2 | Add same keys |
| `packages/web-core/src/i18n/locales/zh-Hant/settings.json` | T2 | Add same keys |
| `packages/web-core/src/i18n/locales/zh-Hans/settings.json` | T2 | Add same keys |
| `packages/web-core/src/shared/dialogs/settings/settings/McpServerDialog.tsx` | T4 | Extend props; add assignments state + default + re-seed; add compat helper; add UI section; update validate(); update result type |
| `packages/web-core/src/shared/dialogs/settings/settings/McpSettingsSection.tsx` | T5 | Simplify openDialog; delete toggleAssignment; replace inline grid with badge summary |

**No changes to**: `shared/types.ts`, any Rust crate, `packages/remote-web`, generated files.
