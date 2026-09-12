# UI contract: mobile workspace right drawer

For the existing mobile tab with id `git`:

- visible wider-mobile label: `Sidebar`;
- accessible name: `Right sidebar`;
- icon: the established right-sidebar (mirrored sidebar) metaphor;
- selected state: `aria-pressed="true"` exactly when `mobileActiveTab` is
  `git`;
- activation: invoke `onMobileTabChange('git')`;
- availability: local workspace navigation exposes it only when a workspace is
  selected and create mode is inactive;
- compatibility: stored `git` values and the `WorkspacesLayout` rendering route
  remain unchanged.

No HTTP, websocket, Rust/TypeScript generated type, or database contract changes.
