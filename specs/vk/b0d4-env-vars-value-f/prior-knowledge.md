# Prior knowledge — vk/b0d4-env-vars-value-f

Searched `vibe-kanban/wiki` and `vibe-kanban/docs/knowledge-base` read-only for
`environment variable`, `env vars`, `1Password`, and `op://`.

- `docs/knowledge-base/workspace-environment-inheritance.md` (tasks
  `6d24-org-env-vars-are`, `5e29-vk-github-fine-g`): workspace terminals have a
  separate process boundary from agents/scripts/dev servers. Extend the single
  workspace-scoped resolver, inject explicitly, filter reserved names, and apply
  process-owned values last. Never write secret .env files or mutate server env.
  Existing remote fetching is best-effort; explicit provider lookup failures
  require a new fail-closed result contract without changing unrelated fetching.
- `docs/knowledge-base/agent-facing-fail-loud-boundaries.md`: report bounded,
  actionable failures at the relevant operation; avoid success-shaped fallbacks.
- `wiki/self-hosted-deployment.md`: application releases are immutable artifacts;
  homelab/modules/vibe-kanban-rebuild.nix owns hosting. No deployment redesign.
- `docs/knowledge-base/cli-tool-oauth-login.md`: 1Password CLI already exists in
  the managed tool catalog; interactive shell sessions are not service-account
  authentication. Check both host and app-installed CLI locations.

Code confirms `resolve_org_env_vars` is consumed by local launches, cluster
launches, and terminal routes. Storage remains a string map. SpecKit command
files explicitly locate artifacts in homelab/specs/vk/b0d4-env-vars-value-f;
implementation belongs in the separate vibe-kanban repository.
