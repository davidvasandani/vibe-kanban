# Internal Preview Settings Contract

`usePreviewSettings(workspaceId)` continues to own the workspace-scoped preview
scratch record and adds:

- `currentRoute: string | null` — last durable auto-detected route.
- `setCurrentRoute(route: string): void` — immediate durable update that preserves
  the override and viewport fields.

Contract invariants:

1. `currentRoute` is either absent or a same-application route beginning with
   `/` and including any query and fragment.
2. Preview-only parameters are removed before calling `setCurrentRoute`.
3. Manual-override navigation does not call `setCurrentRoute`.
4. `clearOverride()` clears only `url`; it does not delete unrelated preview
   settings.
5. Every write emits a complete `PREVIEW_SETTINGS` payload for compatibility
   with the existing scratch update API.
