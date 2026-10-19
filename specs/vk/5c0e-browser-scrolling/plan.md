# Technical plan
Applied `/speckit.plan`.

Add BrowserAction::Scroll { x, y, delta_x, delta_y } in crates/services/src/services/browser/types.rs and map it to Input.dispatchMouseEvent mouseWheel in cdp.rs. All fields are i32, matching existing wire-enum constraints. Generate shared/types.ts through the existing generator. No new persistence or endpoints.

Extend packages/web-core/src/pages/workspaces/BrowserPanelContainer.tsx: use object-contain geometry for coordinates/deltas; a native non-passive wheel listener prevents local scrolling. Normalize line units to 16 CSS pixels and page units to the displayed frame dimensions. Touch/pen pointer capture starts only within the frame, tracks an 8px threshold, reverses displacement for scrolling, suppresses click/focus after swipes, and cancels on multi-touch, pointercancel or context changes. Use the initial remote anchor for nested scroll containers. Wheel/touch work must stay bounded and never replay after control changes.

Add rendered component tests in BrowserPanelContainer.test.tsx and protocol/control/real-Chromium coverage in browser Rust tests. Run formatting, frontend checks/lint, relevant Rust tests and independent Codex review.

Constitution: II contract tests, III/VI existing action gate reuse, IV shared frontend behavior, control-generation isolation. No deviation. Physical touch behavior remains a separate phone check.
