# Analysis: Settings-Owned MCPs in Every New Session

## Cross-Artifact Check

| Concern | Spec | Plan | Tasks | Result |
|---|---|---|---|---|
| Settings is sole MCP authority | FR-001, FR-009 | Steps 1, 7 | T001, T009 | Covered |
| All MCP-capable executors | FR-001 | Steps 1–4 | T001–T004 | Covered |
| Execution-scoped native config | FR-003–FR-005 | Steps 2–4 | T002–T006 | Covered |
| Auth/runtime asset preservation | FR-004 | Steps 2–3 | T002, T003, T008 | Covered |
| Secret-safe authenticated snapshot | FR-002, FR-006 | Steps 1, 4 | T001, T004, T008 | Covered |
| Codex-only confirmed live refresh | FR-007 | Step 5 | T005, T008 | Covered |
| Concurrent isolation and cleanup | FR-003, FR-008 | Steps 2–3 | T003, T006, T008 | Covered |
| Homelab competing entry removal | FR-009 | Step 7 | T009 | Covered |
| Verification and independent review | Success measures | Step 8 | T010, T011 | Covered |
| Reusable knowledge and delivery | Success measures | Step 8 | T012, T013 | Covered |

## Constitution Check

- **III / VI — generalize existing machinery**: the plan extends the Codex
  snapshot and native adapter paths rather than adding a second config system.
- **II — test the contract**: producer, consumer, isolation, cleanup, mismatch,
  and refresh behavior have explicit regression tasks.
- **XVII — confirmed live capability**: non-Codex executors adopt settings at a
  new process boundary; the plan does not mislabel file writes as live refresh.
- **XXIII — authoritative snapshots**: the coordinator resolves settings and the
  authenticated dispatch carries the bounded executor-bound value.
- **XXIV — one MCP authority**: deployment and repository seeding are removed.
- **XXVIII — executor-neutral isolation**: native config is execution-scoped and
  child-only environment overrides preserve worker-global state.

No constitution violations found.

## Risk Review

1. **Home overlay escapes or broken links**: require the native target to be
   structurally below source home and create links without following targets.
2. **XDG bypasses scoped HOME**: provide a scoped XDG root when the adapter target
   is under `.config`.
3. **Cleanup removes shared data**: cleanup owns only the explicit execution root;
   linked targets are never traversed for deletion.
4. **Secrets in errors**: errors name executor/path operation only, never server
   definitions or serialized snapshot values.
5. **Rolling deployment**: snapshot remains optional and uses the existing wire
   type, avoiding a protocol break.

## Conclusion

The specification, plan, and tasks are complete, mutually consistent, and ready
for implementation. No open question requires user input.
