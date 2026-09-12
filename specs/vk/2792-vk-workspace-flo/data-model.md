# Data Model: Hide Workspace Context Bar on Mobile Layout

No database entities, API DTOs, generated TypeScript types, persistence keys, or
saved preference schemas change for this feature.

The context bar's existing persisted desktop snap position remains unchanged in
the UI preferences store. Mobile hiding must not delete, migrate, or rewrite the
saved value.

## Implementation-Only Policy Input

The feature may add a private frontend helper to make the visibility rule
testable:

```typescript
type WorkspaceContextBarVisibilityInput = {
  isResponsiveMobile: boolean;
  isRealMobileDevice: boolean;
};
```

Derived behavior:

```typescript
shouldRenderWorkspaceContextBar(input) =
  !input.isResponsiveMobile && !input.isRealMobileDevice
```

Invariants:

- `isResponsiveMobile` is sourced from `useIsMobile()`.
- `isRealMobileDevice` is sourced from `isRealMobileDevice()` or the existing
  hook equivalent.
- The helper is not a shared generated type.
- The helper does not introduce persistence.
- The helper does not change `ContextBarPosition` values or defaults.

## Existing State Left Intact

| State | Location | Change |
| --- | --- | --- |
| Desktop context-bar snap position | `useUiPreferencesStore` context bar position state | None |
| Workspace mobile active tab | `useMobileActiveTab()` | None |
| Context-bar action definitions | `ContextBarActionGroups` | None |
| Workspace/session/chat data | existing workspace providers and stores | None |
