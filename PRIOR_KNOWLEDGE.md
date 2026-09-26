# Prior knowledge: model menu (vk/6823-model-menu)

Distilled from the project knowledge base (`wiki/`) — read-only recall.

## executor-model-catalogs.md
- Model pickers are populated from each executor's `discover_options`; the Rust
  catalog (`default_discovered_options` in `claude.rs`) is the single source of
  truth. The frontend must not keep a duplicate list.
- Keep catalogs newest-first; when the user supplies the desired menu, remove
  superseded entries instead of treating the change as additive.
- Cover catalogs with an **exact ordered** regression test over IDs, labels and
  reasoning option IDs so future changes are deliberate.
- Claude's launch command, picker and context-window inference
  (`context_window_for_model`) are separate contracts; the `opus` alias already
  resolves to Opus 5.5 with the pinned CLI (2.1.281). Explicit
  `claude-opus-5-5` reports a 1M context window.
- Do not bundle catalog changes with a CLI pin bump (Renovate-managed).

## workspace-context-bar-responsive-visibility.md
- Two distinct mobile signals: `useIsMobile()` (layout, `max-width: 767px`)
  and `isRealMobileDevice()` (user agent). They can disagree (iOS PWA may
  report non-mobile UA while in the mobile layout). For touch-specific policy,
  treat *either* signal as mobile (defense in depth).

## workspace-carousel-view.md
- Chat editors autofocus on mount; focus is not a user signal. Relevant
  background: on mobile, automatic focus of text inputs raises the software
  keyboard. The command-bar `SelectionDialog` already skips autofocus on
  `isRealMobileDevice()` for exactly this reason (precedent for R1).

## specs/opus-5-5-verification.md
- `claude-opus-5-5` is a valid explicit model ID for the pinned CLI and is in
  the embedded model catalog; Opus 5 stays selectable.

## Gaps (no prior page)
- No page covers profile-level persisted picker preferences
  (`recently_used_models` in `ExecutorProfile`, merged via
  `merge_with_defaults` / `compute_overrides`). Candidate for a new page after
  this task.
