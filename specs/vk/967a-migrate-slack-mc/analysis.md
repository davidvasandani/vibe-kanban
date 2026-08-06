# SpecKit Analysis: Shared HTTP Slack MCP

**Run:** `/speckit.analyze` on 2026-08-06
**Artifacts:** `spec.md`, `plan.md`, `tasks.md`, contracts, both project
constitutions

## Findings

1. **ERROR — `tasks.md` ordering:** T010 places `/speckit.analyze` after
   implementation verification, contradicting the mandated pipeline and its own
   “before implementation completion” wording. Move analysis to the pre-
   implementation gate and renumber/retitle final controls accordingly.
2. **WARNING — `plan.md` supervised environment:** The upstream server and npm
   launcher use cache/home paths, but the systemd design mentions a dynamic user
   without specifying `HOME`, writable cache/state directories, or the exact
   executable path. This violates the supervised-process environment principle
   unless the module declares those paths explicitly.
3. **WARNING — `plan.md` artifact integrity ownership:** The plan says the
   deployment module owns the pinned launcher URL/digest but also implies it will
   execute the URL through `npx`, which does not verify the outer tarball before
   code executes. The existing constitution exception is detection-only and must
   be named explicitly in the module/docs; preferably fetch the launcher as a
   fixed-output Nix artifact if practical.
4. **WARNING — `tasks.md` Nix tests:** T006 says “existing homelab test
   convention” but names no exact test file, contrary to the task-template
   requirement. Research the repository's module-evaluation test location and
   update the task before implementation.
5. **WARNING — `spec.md` acceptance environment:** Live coordinator/worker MCP
   handshakes depend on a token file not provisioned in this worktree. Separate
   deterministic repository acceptance (demo credential or isolated stub) from
   post-deploy operator verification so missing external secret access is not
   reported as a passing local test.
6. **INFO — scope/coverage:** FR-1 through FR-13 otherwise map to T001–T009.
   Exact historical migration, custom-entry preservation, secret removal,
   private network policy, optional health, docs, and independent review all
   have implementation and verification work.
7. **INFO — constitutions:** No requested change crosses into another service.
   The design is compatible with private-exposure, runtime-secret, exact vendor
   config editing, distributed execution, and immutable fork principles once
   findings 2–5 are addressed.

## Gate result

**Not ready to implement until findings 1–5 are resolved in the planning
artifacts.** No source implementation was performed during analysis.

