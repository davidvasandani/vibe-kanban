# Implementation Plan: iPad Windowed Layout — Top Nav Cut Off

Single-file change plus verification. See `SPEC.md` for rationale.

## Step 1 — Add top safe-area padding to the shared Navbar

File: `packages/ui/src/components/Navbar.tsx`

### 1a. Mobile navbar variant

The mobile `<nav>` (`mobileMode` branch) currently has no vertical padding on
the element itself (its rows carry `px-base py-half`). Add an inline style that
pads the top by the safe-area inset so the navbar's `bg-secondary` fills the
status-bar area:

```tsx
<nav
  className={cn('flex flex-col bg-secondary border-b shrink-0', className)}
  style={{ paddingTop: 'env(safe-area-inset-top)' }}
>
```

When the inset is `0`, `padding-top: 0` — no visual change.

### 1b. Desktop navbar variant

The desktop `<nav>` uses `px-base py-half`. Keep `py-half` (so the bottom
padding is unchanged) and override the top padding via inline style to add the
inset on top of the existing `0.25rem` (`py-half`) value:

```tsx
<nav
  data-tauri-drag-region
  className={cn(
    'flex items-center justify-between px-base py-half bg-secondary border-b shrink-0',
    className
  )}
  style={{ paddingTop: 'calc(0.25rem + env(safe-area-inset-top))' }}
>
```

Inline styles win over the Tailwind class, so only `padding-top` is overridden;
`padding-bottom` stays `py-half`. When the inset is `0`, top padding resolves to
`0.25rem` — identical to `py-half` today.

A short comment on each `style` explains that `0.25rem` mirrors `py-half` and
that the inset keeps the nav clear of the status bar in windowed/notched
layouts.

## Step 2 — Verify

1. `pnpm run check` — frontend typecheck (and backend, unaffected).
2. `pnpm run lint` — ESLint (no inline-style rule violations expected;
   inline styles are already used in the codebase).
3. `pnpm run format` — Prettier for web packages.
4. Manual/visual reasoning: with the inset `0` the navbar is unchanged; with a
   non-zero inset the navbar content shifts down by the inset and the
   `bg-secondary` fills the gap. In the desktop grid the sibling corner spacer
   (also `bg-secondary`, stretched to row height) covers the left portion of the
   strip so there is no color seam.

## Step 3 — Independent Codex review

Run the `codex-review` skill / Codex CLI against the diff, iterate on any
confirmed findings, and re-verify before marking the task ready.

## Risk / rollback

- Single, additive change confined to one component's two root `<nav>` elements.
- Zero behavioral change on displays without a top inset (the common case).
- Rollback = remove the two `style` props.
