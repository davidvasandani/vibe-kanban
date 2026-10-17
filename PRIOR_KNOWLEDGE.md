# Prior knowledge — vk/5c0e-vk-browser

Searched the project's `vibe-kanban/wiki` and `docs/knowledge-base` for browser, clipboard, and keyboard.

- `wiki/browser-session-control-arbiter.md`: all browser actions share the control arbiter and command gate. Check generation before and after queueing; never bypass this for clipboard operations. Driver handles isolate CDP; Type already uses Input.insertText. Keep text out of audit rows. Mock driver tests cover control without Chromium.
- `wiki/browser-title-selection.md`: browser session UI uses existing live session state and shared containers.
- `docs/knowledge-base/responsive-flex-toolbars.md`: controls must remain reachable in narrow layouts; allow wrapping.
- `docs/knowledge-base/locale-key-consistency.md`: keep translated keys aligned across locales.

The knowledge base is populated. This stage made no changes to it. The initial SPEC.md predates this recall as required by stage order; these constraints refine its technical direction for planning.

Additional relevant match discovered while reading the workspace constitution: `homelab/docs/knowledge/pasting-into-cdp-screencast-live-views.md` describes real-editor gesture focus, composition, native paste events, explicit Clipboard API fallback, and distinguishing touch emulation from physical mobile verification. Its Firecrawl-specific implementation is outside this task's scope; reuse the lessons only.
