# Implementation Plan: Keep Mobile Toolbar Leading Tool Visible

**Spec**: `./spec.md`
**Status**: Ready

## Technical Context

React/TypeScript shared `Navbar` in `packages/ui`, tested through web-core
Vitest. No API, state, dependency, or data-model change.

## Approach

Keep the outer mobile workspace region as `flex-1 min-w-0`, remove overflow
from it, and move `overflow-x-auto min-w-0` to the inner tool group. Fixed
leading navigation becomes `shrink-0`; trailing actions already are. The tool
group retains flexible growth, with a minimum-width inner row whose tools share
surplus space.

## Constitution Check

The change is localized, reversible, presentation-owned, and protected by a
rendered real-component test. No violations.

## Verification

Focused Vitest, UI/web-core typecheck, UI lint, formatting, diff validation, and
independent Codex review.
