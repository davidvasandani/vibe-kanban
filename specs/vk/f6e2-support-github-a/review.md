# Independent Codex review

Task: VAS-966. Command: `codex review --uncommitted`, read-only review.

1. Found that explicit `BACKEND_PORT=0` / `PORT=0` exported port zero to local sessions. Fixed by reading the bound port file for zero or absent configured ports, preserving explicit base URLs and the bound host. Added a regression test for automatic assignment, explicit ports, and failed discovery.
2. Found that a global mint lock could block other owners behind a slow lookup. Replaced it with per-row locks and a short-lived map lock. Health probes share the row's mint lock. Added a concurrency test that stalls one owner and verifies both cached and uncached other owners remain usable.
3. Final result: "No actionable regressions were found in the current changes." Review covered storage, refresh, routing, API and Settings. The reviewer did not run tests in its read-only environment; implementation validation is recorded separately.
