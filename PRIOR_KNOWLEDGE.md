# Prior Knowledge — recalled for `vk/a96d-electric-sync-er`

Searched the project knowledge base (`wiki/`) for pages relevant to this task
(Electric client-side sync, REST fallback, sync-error surfacing / banner).

## Relevant findings

**None directly relevant.** The knowledge base currently has two pages, and
neither covers the frontend ElectricSQL hybrid-sync layer this task modifies:

- `wiki/mobile-kanban-scrolling.md` — mobile kanban touch scrolling/snapping
  CSS. Unrelated.
- `wiki/self-hosted-deployment.md` — deploy pipeline (`VK_RELEASES_DIR`, homelab
  NixOS units). Mentions the *server-side* `electric_sync` Postgres role during
  ElectricSQL startup, but says nothing about the client hybrid-sync/fallback
  behaviour or how sync errors reach the UI.

So on the topic of **client-side Electric sync + REST fallback + sync-error
banner**, this is effectively a first task — no prior page to build on.

## Context gathered from code (for spec/plan, not from the KB)

- Hybrid sync lives in
  `packages/web-core/src/shared/lib/electric/collections.ts`:
  `createHybridSync` tries Electric, and on a 3000ms readiness timeout (or a
  network/5xx failure) locks the source to a REST `fallbackUrl` poller
  (`createFallbackSync`, 30s interval).
- Errors flow: `reportError` → `config.onError` → `useShape` `setError`
  (`shared/integrations/electric/hooks.ts`) → `SyncErrorContext`
  (`shared/providers/SyncErrorProvider.tsx`) → navbar banner
  (`NavbarContainer.tsx`, `remote-web/.../RemoteWorkspaceRail.tsx`).
- Backend/proxy background: `crates/remote/AGENTS.md` — Electric is a read-path
  sync engine behind an auth-gated proxy; writes go through REST; the frontend
  awaits `txid` on the Electric stream.

## Knowledge to capture after shipping

A new `wiki/` page on the client Electric hybrid-sync + fallback design and the
"fallback is recovery, not an error" principle (see stage 5).
