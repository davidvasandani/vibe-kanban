# Prior knowledge — vk/5c0e-vk-browser scrolling follow-up

Searched wiki and docs/knowledge-base for browser and scrolling. The knowledge base is populated; this stage is read-only there.

- wiki/browser-session-control-arbiter.md: extend the guarded BrowserAction protocol and integer coordinates. Input must remain bound to session, connection and control generation. Touch taps currently focus the keyboard synchronously; swipes must avoid that path.
- wiki/mobile-kanban-scrolling.md: distinguish deliberate touch interaction from enclosing page scroll; scrolling authority belongs to the intended surface.
- Existing BrowserPanelContainer maps object-contain letterboxing for click/move but has no wheel or swipe handler. Protocol inspection confirms scroll is missing; add a typed Scroll variant using the same command gate.
