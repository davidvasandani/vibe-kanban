# Implementation Plan: VK MCP Management UX

Derived from `specs/vk/76d1-vk-mcp-ux/spec.md`, `contracts.md`, and `research.md`.
Follows constitution Principle X (dialogs hold provisional state) and Principle III (small, reversible steps).

---

## Prerequisites

- Read `contracts.md` before touching any file — all interface changes are specified there.
- Run `pnpm run check` and `pnpm run lint` green before starting (establish baseline).
- No backend changes are expected; flag any discovered necessity in a code comment.

---

## Step 1 — Extend McpServerDialog props and seed assignments state

**File**: `packages/web-core/src/shared/dialogs/settings/settings/McpServerDialog.tsx`

### 1a. Update imports
Add to import block:
```typescript
import type { BaseCodingAgent, JsonValue, SharedMcpProfile } from 'shared/types';
import { codecForAgent } from '@/shared/lib/mcpServerCodec';
import { toPrettyCase } from '@/shared/lib/string';
```
(Some may already be present; add only what's missing.)

### 1b. Extend interface (contracts.md §1)
Replace `McpServerDialogProps` and `McpServerDialogResult` with the new signatures from `contracts.md §1`.

### 1c. Add `assignments` local state
Inside `McpServerDialogImpl`, add:
```typescript
const [assignments, setAssignments] = useState<BaseCodingAgent[]>(
  initial?.assignments ?? []
);
```

### 1d. Extend the NiceModal re-seed useEffect
The existing `useEffect` (currently re-seeds name/form/argsText/customJson/error on `modal.visible`) must also reset `assignments`. Capture the reset form value in a local variable first so the transport is available for the default calculation:
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
Add this block to the existing effect body. No new effect needed.

### 1e. Derive `currentTransport` for compatibility
Below the existing `isCustom` constant, add:
```typescript
const currentTransport: McpTransport | null = isCustom ? null : form.transport;
```

Note: `isTransportCompatible` must be imported from `mcpServerCodec.ts` (added in Step T1) before this file is edited.

### 1f. Add `incompatibilityReason` inline helper
Inside the component body (after `const { t } = useTranslation('settings')`):
```typescript
const getIncompatibilityReason = (executor: BaseCodingAgent): string | null => {
  if (currentTransport === null) return null;
  const supported = codecForAgent(executor).transports;
  if (supported.includes(currentTransport)) return null;
  if (currentTransport === 'sse') return t('settings.mcp.dialog.incompatible.noSse');
  if (currentTransport === 'http') return t('settings.mcp.dialog.incompatible.noHttp');
  return t('settings.mcp.dialog.incompatible.generic', { transport: currentTransport });
};
```
This helper depends on `currentTransport` (which tracks form state) so it re-evaluates on every render when the user changes transport.

### 1g. Add assignment validation to `validate()`
At the **end** of `validate()`, just before `return { name: trimmedName, entry }`:
```typescript
if (assignments.length === 0) {
  setError(t('settings.mcp.validation.assignmentRequired'));
  return null;
}
```

### 1h. Change `validate()` return type and `handleSave`
`validate()` currently returns `{ name, entry } | null`. Change to return `{ name, entry, assignments } | null`:
```typescript
// At the end of validate(), change:
return { name: trimmedName, entry };
// To:
return { name: trimmedName, entry, assignments };
```

`handleSave` calls `modal.resolve(result)` — it already passes the whole result object, so no change needed there beyond the `validate()` return type.

### 1i. Add agent assignment UI to the dialog body
After the transport/command/url fields (i.e., after the existing `{error && ...}` block, before `<DialogFooter>`):

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

Place this UI block **above** the `{error && ...}` Alert, so the error always appears last before the footer.

---

## Step 2 — Update McpSettingsSection to pass new props and remove inline grid

**File**: `packages/web-core/src/shared/dialogs/settings/settings/McpSettingsSection.tsx`

### 2a. Update `openDialog` callback (contracts.md §5)
Replace the existing `openDialog` (lines 384–425) with the simplified version from `contracts.md §5`. Key changes:
- Add `profiles` to the `McpServerDialog.show(...)` call.
- Add `assignments: server.assignments` to `initial` when editing.
- Remove the post-dialog assignment derivation logic (the `const assignments = ...` block).
- Use `result.assignments` directly when calling `setServer(...)`.

### 2b. Delete `toggleAssignment` callback
Delete lines 448–467 (the `toggleAssignment` callback). It is fully replaced by modal-local state.

### 2c. Replace the inline assignment grid with compact badges
In the server card JSX, replace the block at lines 1022–1069 (the `<div className="mt-2 grid gap-1 ...">` containing all the profile checkboxes) with the compact badge summary from `contracts.md §7`.

Preserve all surrounding card structure:
- Server name + transport badge + auth badge (lines 881–908) — unchanged.
- Assignment count `{server.assignments.length} assignments` (line 905–908) — can be removed now that badges are shown, or kept as a fallback for zero-assignment state. Remove the bare count line; the badges convey the same information more directly.
- Test/Edit/Delete button row (lines 909–970) — unchanged.
- `TestResultDetails` block (lines 973–998) — unchanged.
- Secondary error list for multiple results (lines 1000–1020) — unchanged.

### 2d. Ensure `profiles` is available to `openDialog`
`profiles` is already derived at line 300:
```typescript
const profiles = readModel?.profiles.filter((profile) => profile.supports_mcp) ?? [];
```
It's used in the existing `openDialog` callback dep array. No change needed here — just confirm `profiles` is in the `useCallback` dependency array for the updated `openDialog`.

---

## Step 3 — Add i18n keys to all 7 locale files

**Files**: all 7 `settings.json` locale files under `packages/web-core/src/i18n/locales/`.

### 3a. English (en/settings.json) — full translations

Under `settings.mcp.dialog`, add:
```json
"agentsTitle": "Assign to agents",
"agentsHelper": "Choose which coding agents should use this MCP server.",
"incompatible": {
  "noSse": "This agent does not support SSE transport.",
  "noHttp": "This agent only supports stdio (command) transport.",
  "generic": "This agent does not support {{transport}} transport."
}
```

Update `settings.mcp.dialog.description`:
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

### 3b. Non-English locales (fr, es, ja, ko, zh-Hant, zh-Hans)

Add the same keys. Use safe neutral English fallbacks or direct translations as appropriate. Every locale file must have every key — no runtime key misses permitted.

Suggested fallback strategy for locales without dedicated translations:
- Copy English strings verbatim for now; mark with a comment in the PR description so translators can update.
- Do not leave any key absent.

---

## Step 4 — Add focused tests

**New file**: `packages/web-core/src/shared/dialogs/settings/settings/McpServerDialog.test.ts`

Test the pure helper and assignment logic. Since no DOM component test infrastructure exists for this surface, use pure-function tests only (compatible with existing Vitest setup).

```typescript
// Test: incompatibilityReason logic (exported or extracted for testability)
// Test: assignment validation — empty assignments returns error string
// Test: assignments seeded correctly for edit vs add
```

If `getIncompatibilityReason` is kept as a component-internal arrow function, extract it to a module-level pure function that accepts `(executor, transport, t)` so it can be unit-tested. Import `t` as a no-op stub in tests.

Alternatively, extract the transport-compatibility check to a new exported pure function in `mcpServerCodec.ts`:
```typescript
// mcpServerCodec.ts (new export)
export function isTransportCompatible(
  executor: BaseCodingAgent,
  transport: McpTransport
): boolean {
  return codecForAgent(executor).transports.includes(transport);
}
```
Then test `isTransportCompatible` in `mcpServerCodec.test.ts`:
```typescript
it('allows CODEX streamable HTTP assignments', () => {
  expect(isTransportCompatible(BaseCodingAgent.CODEX, 'http')).toBe(true);
});
it('reports compatibility for CLAUDE_CODE with sse', () => {
  expect(isTransportCompatible(BaseCodingAgent.CLAUDE_CODE, 'sse')).toBe(true);
});
it('reports incompatibility for GROK with sse', () => {
  expect(isTransportCompatible(BaseCodingAgent.GROK, 'sse')).toBe(false);
});
```

---

## Step 5 — Verify and format

1. **Run `pnpm run check`** — TypeScript type checks for all frontend workspaces.
2. **Run `pnpm run lint`** — ESLint across web-core and local-web.
3. **Run `cargo test --workspace`** (if any Rust touched, which is not expected).
4. **Run `pnpm run format`** — Prettier + rustfmt. Must produce no diff.
5. **Vitest**: run `pnpm run test` or the targeted test file; confirm all existing and new tests pass.
6. **Visual check** (if local env available): start `pnpm run dev`, open Settings → MCP Servers, verify:
   - Cards show agent badges (not checkboxes).
   - Add dialog shows agent assignment section.
   - Selecting no agents and clicking Add shows validation error.
   - Canceling Edit leaves draft unchanged.
   - Renaming a server replaces it without duplicate.
   - OAuth Connect remains accessible from card when `auth_required`.
   - JSON mode still round-trips `SharedMcpDraftServer[]`.
   - Narrow layout (~480 px) is usable.

---

## Dependency / Risk Notes

| Risk | Mitigation |
|------|-----------|
| NiceModal instance reuse: if `profiles` or `initial.assignments` change between opens, stale values could be shown | The `useEffect` on `modal.visible` re-seeds all state on every open — verify `profiles` and `initial` are passed fresh to `McpServerDialog.show()` each call, not captured in a stale closure |
| `currentTransport` derivation for compatibility: for custom JSON (`isCustom`), transport is `null` → all agents selectable | Covered by `isCustom ? null : form.transport` pattern in contracts |
| `validate()` runs assignment check before `modal.resolve(result)` — result shape changed | Confirm `McpServerDialogResult` type update propagates: `defineModal<McpServerDialogProps, McpServerDialogResult>` at bottom of file |
| Default assignments for a new Add: must pre-select exactly ONE compatible agent | Seed the first compatible profile using `.filter(...).slice(0, 1)` in the `useEffect` re-seed (see Step 1d). This mirrors the existing `openDialog` `.slice(0, 1)` heuristic and matches FR-5. Do NOT pre-select every compatible profile. |
| i18n key completeness | Check all 7 locale files manually after adding keys |

---

## Files Changed (Summary)

| File | Change |
|------|--------|
| `packages/web-core/src/shared/dialogs/settings/settings/McpServerDialog.tsx` | Extend props; add assignments state + re-seed; add compatibility helper; add assignment UI section; update validate(); update result type |
| `packages/web-core/src/shared/dialogs/settings/settings/McpSettingsSection.tsx` | Simplify openDialog; delete toggleAssignment; replace inline grid with badge summary |
| `packages/web-core/src/shared/lib/mcpServerCodec.ts` | Export `isTransportCompatible` (new pure function, ~4 lines) |
| `packages/web-core/src/shared/lib/mcpServerCodec.test.ts` | Add 3 tests for `isTransportCompatible` |
| `packages/web-core/src/i18n/locales/en/settings.json` | Add 6 new keys, update 1 key |
| `packages/web-core/src/i18n/locales/fr/settings.json` | Add same keys (translated or fallback) |
| `packages/web-core/src/i18n/locales/es/settings.json` | Add same keys |
| `packages/web-core/src/i18n/locales/ja/settings.json` | Add same keys |
| `packages/web-core/src/i18n/locales/ko/settings.json` | Add same keys |
| `packages/web-core/src/i18n/locales/zh-Hant/settings.json` | Add same keys |
| `packages/web-core/src/i18n/locales/zh-Hans/settings.json` | Add same keys |

**No changes to**: `shared/types.ts`, any Rust crate, `packages/remote-web`, generated files.
