# Spec: iPad Windowed Layout — Top Nav Cut Off

## Problem

When the Vibe Kanban web app is viewed on an iPad in a **windowed / Stage
Manager layout** (or any environment that exposes a non-zero top safe-area
inset), the top navigation bar is clipped by the device status bar. Its
content (title, tabs, action icons) renders underneath the status bar and is
partially or fully cut off.

## Root cause

`packages/local-web/index.html` declares:

```html
<meta name="viewport" content="width=device-width, initial-scale=1.0, viewport-fit=cover" />
```

`viewport-fit=cover` instructs the browser to lay the page out edge-to-edge,
extending **under** the device safe areas (status bar, home indicator, rounded
corners). To keep content clear of those areas an app must add padding using
the CSS `env(safe-area-inset-*)` values.

The app currently only compensates for the **bottom** inset:

- `packages/web-core/src/shared/components/ui-new/containers/SharedAppLayout.tsx`
  → mobile container uses `pb-[env(safe-area-inset-bottom)]`.
- `packages/remote-web/src/app/layout/RemoteAppShell.tsx` → same.
- `packages/ui/src/components/MobileDrawer.tsx` → same.

Nothing accounts for `env(safe-area-inset-top)`. On devices/layouts where that
inset is `0` (typical desktop, non-windowed) there is no visible problem, which
is why it went unnoticed. On iPad windowed mode the inset is non-zero and the
top nav is cut off.

## Goal

The top navigation bar must always render fully below the top safe-area inset,
regardless of platform, while remaining visually unchanged on displays where
the inset is `0`.

## Approach

Apply top safe-area padding to the shared `Navbar` component
(`packages/ui/src/components/Navbar.tsx`) — the element that is flush with the
top of the viewport in every layout that renders it (local desktop, local
mobile; the remote shell renders the same component through
`RemoteNavbarContainer`).

Padding the navbar itself (rather than the outer app container) is preferred
because:

- The navbar background (`bg-secondary`) then fills the status-bar area,
  matching the standard iOS convention where the status bar blends with the
  navigation bar. Padding the outer `bg-primary` container instead would leave
  a mismatched primary-colored strip above the navbar.
- It is the smallest, most localized change and touches the single element that
  is actually being clipped.
- In the local desktop grid the navbar shares its row with a `bg-secondary`
  corner spacer that stretches to the row height, so the whole top strip renders
  `bg-secondary` seamlessly.

### Why an inline `style` (not a Tailwind arbitrary class)

The existing bottom-inset padding uses Tailwind arbitrary values
(`pb-[env(safe-area-inset-bottom)]`), but those live in the **app shells**,
which each app's Tailwind config scans. The `Navbar` lives in
`packages/ui/src`, which the `remote-web` Tailwind config does **not** scan (and
it does not define the `spacing.half` token). A new arbitrary utility added in
`ui/src` would therefore not be generated for the remote build and, if it
referenced `theme(spacing.half)`, could error. A plain inline `style` with a
`calc(...)` value is evaluated natively by the browser, needs no class
generation, resolves identically in both apps, and matches inline-style usage
already present in the codebase (e.g. `style={{ minWidth: 56 }}` in
`SharedAppLayout`).

### Behavior contract

- **Desktop navbar** (`px-base py-half`): top padding becomes
  `calc(0.25rem + env(safe-area-inset-top))` where `0.25rem` is the existing
  `py-half` value. The bottom padding stays `py-half`. When the inset is `0`
  the top padding equals `0.25rem` — identical to today.
- **Mobile navbar** (no vertical padding on the `<nav>` itself; inner rows own
  their spacing): top padding becomes `env(safe-area-inset-top)`. When the inset
  is `0` this is `0` — identical to today.

In both cases the navbar's `bg-secondary` extends up into the status-bar area.

## Out of scope

- Left/right safe-area insets (iPads have no side notch; Stage Manager windows
  are inset from screen edges by the OS).
- The `CloudShutdownExportBanner` (disabled by default in this fork) — the
  navbar is the topmost element in the default configuration.
- Any change to `viewport-fit` or the existing bottom-inset handling.

## Acceptance criteria

1. On a layout with a non-zero top safe-area inset (iPad windowed), the full top
   nav is visible below the status bar; the status-bar area shows the navbar's
   `bg-secondary` color.
2. On displays where the top inset is `0`, the navbar is pixel-identical to
   before (top padding unchanged).
3. Applies to both the desktop and mobile navbar variants.
4. `pnpm run check` and `pnpm run lint` pass.
