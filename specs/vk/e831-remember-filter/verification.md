# Verification

Regression tests cover resolver precedence, empty/false overrides, project-scoped tags, relative Me, malformed data, user/view/project isolation, browser remounts, transient search, delayed initialization, personal/site separation, reset, and failed saves.

Self-hosted Vibe Kanban stores explicit personal/site defaults in dedicated server scratch records. Remembered per-view changes stay in the browser. Remote-web labels personal defaults as browser-only and does not expose a site-wide action because its scratch backend is browser storage. Existing scratch access permissions are unchanged.

- `pnpm install --frozen-lockfile`: passed.
- Targeted Vitest suite: 31 tests passed across resolver, hook, existing filter matching and rendered Filters dialog.
- `pnpm run format`: passed; final touched files additionally formatted after review fixes.
- All frontend typechecks passed (local-web, remote-web, web-core, ui).
- All frontend lint checks passed (local-web, web-core, remote-web, ui).
- Full `pnpm run check` reached backend compilation but failed in vendored OpenSSL: `No rule to make target crypto/cmac/build.info, needed by configdata.pm`. No Rust changes are included.
- Independent Codex review round 1 found that search could pin inherited defaults; corrected with separate transient search state and a regression test. Round 2 found malformed enclosing browser records could throw before validation; corrected and regression-tested. Round 3 reported no concrete regressions. Independent review was source-only; the author ran the validation above.
