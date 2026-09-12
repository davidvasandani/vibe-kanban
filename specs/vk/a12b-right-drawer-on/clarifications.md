# Clarifications: discoverable mobile workspace right drawer

## 1. Visible and accessible naming

**Decision:** Use `Sidebar` as the visible tab label and `Right sidebar` as its
accessible name.

**Reasoning:** `Git` describes only part of the existing shared drawer and is
the source of the discoverability problem. `Right sidebar` is clearest to
assistive technology, while the shorter visible label respects the constrained
mobile tab strip.

## 2. Workspace-less and create-mode behavior

**Decision:** Omit the right-sidebar destination unless a workspace is selected
and the screen is not in create mode.

**Reasoning:** The shared drawer needs workspace data. A disabled or empty tab
would falsely suggest useful content and consume narrow navigation space.

## Remaining questions

None.

