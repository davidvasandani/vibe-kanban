# Technical plan
Command: /speckit.plan
Spec: ./spec.md

## Architecture
- `homelab/modules/vibe-kanban-rebuild.nix`: provision a private shared AWS directory and link each service home `.aws` to it before coordinator/worker startup. Add AWS CLI to the worker service path. Use an ordered oneshot migration service, require shared mount, and fail on conflicting coordinator files.
- `homelab/modules/vibe-kanban-aws-state.py`: idempotent directory migration/link helper, coordinator imports existing state, worker preserves local state as backup. Do not print file contents. Ensure normal scoped-home overlay sees the `.aws` link on every new turn.
- `vibe-kanban/crates/worker/src/execution.rs`: add a focused overlay regression demonstrating late AWS login and token replacement are visible through an isolated home.
- `vibe-kanban/packages/web-core/src/i18n/locales/*/settings.json`: available CLI rows explicitly report host scope and unverified agent reachability; AWS copy describes host check and profile selection without promising generic cluster propagation.

## Verification
- Python migration tests for existing config/cache, idempotency, conflict refusal, backup preservation, worker behavior, permissions, and invalid symlinks.
- Nix parse/format and cluster evaluation tests for service ordering, shared paths, and AWS CLI PATH.
- Rust worker overlay test; pnpm setup, format, frontend checks and lint as supported.
- Independent Codex CLI diff review, then live AWS acceptance if a session and deployed change are available. Never log credentials.

## Constitution check
Reuses shared storage and scoped homes (VI), tests contracts (II), explicitly defines vendor credential sharing (VIII), and reports actual evidence (XI). No deviation.

## Dependencies and risk
Two repositories require coordinated rollout. Existing agent turns retain their already-created home overlays; start a new turn after deployment. SSO cache remains vendor-managed and refresh concurrency is unchanged. Shared service identity can access all configured SSO profiles, as the settings operator already authorized. No arbitrary coordinator path is forwarded to unconfigured workers.
