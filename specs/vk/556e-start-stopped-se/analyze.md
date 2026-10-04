# Cross-check: spec.md / plan.md / tasks.md / constitution

No errors or coverage gaps found.

- Every functional requirement (FR-001..FR-005) in `spec.md` maps to at
  least one task in `tasks.md`: FR-001/FR-003 → T003; FR-002 → T002/T003
  (the `!interruptedNotice` gate keeps the footer path unchanged); FR-004 →
  T003 (footerRight/`Send` untouched by design, verified by T011's
  non-interrupted assertions); FR-005 → T004–T010, T012.
- Every acceptance scenario has corresponding Vitest coverage planned in
  T011 (AC-001/AC-002 for the interrupted case, AC-003 for the
  non-interrupted case) and a manual-check task (T013) for the parts
  automated coverage can't reach (live visual confirmation).
- `plan.md`'s Design section references only real files/props confirmed by
  reading the source (`SessionChatBox.tsx`, `SessionChatBoxContainer.tsx`,
  `ChatBoxBase.tsx`) — no invented APIs.
- Constitution: reviewed and appended (`Review: vk/556e-start-stopped-se`,
  no amendment). Principles II, III, IV, VI apply as written; none are
  stretched or contradicted.
- Info/warning-level notes (non-blocking):
  - **I1**: `spec.md`'s AC-004 names a "manual locale-key-set diff" rather
    than `scripts/check-i18n.sh` directly, because that script clones
    `origin/main` over the network for its baseline literal-string count,
    which may not be reachable from this environment. `tasks.md` T012/T013
    reflect the same split (local check now, full script when network/CI
    is available). This is an environment constraint, not a spec gap.
  - **I2**: T013's live visual check could not complete — the workspace
    browser tool cannot reach this sandbox's local dev server at all (not
    just port 3000; its own backend health endpoint on port 3001 also comes
    back `net::ERR_CONNECTION_REFUSED` from that tool, while a plain `curl`
    from this shell reaches both ports fine), which is a pre-existing
    environment networking limitation unrelated to this change. All other
    verification (typecheck, lint, the new and existing Vitest coverage,
    `cargo check --workspace`, i18n key-set consistency) completed clean.
    This is disclosed rather than worked around.
