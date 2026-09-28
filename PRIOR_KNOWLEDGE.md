# Prior knowledge — vk/0f52-manage-gh-token

Read-only search of `vibe-kanban/wiki`, `vibe-kanban/docs/knowledge-base`,
`homelab/docs`, `homelab/docs/knowledge-base`, and the earlier SpecKit
artifacts. Search terms: `GH_TOKEN`, `fine-grained`, `PAT`, `github`, `env
vars`, `1Password`, `PATH`, `cluster worker`.

## Directly relevant

- **`specs/vk/5e29-vk-github-fine-g/` + `homelab/docs/vibe-kanban-github-auth.md`**:
  a per-owner `gh` router already ships in
  `homelab/modules/vibe-kanban-rebuild.nix`
  (`services.vibe-kanban-rebuild.githubAuth.orgTokenRefs`). No host sets
  `orgTokenRefs`, so the router is not installed anywhere. It is Nix-only:
  every change needs a rebuild and restart, and it has no UI and no Git
  routing. The parts worth reusing are its routing rules:
  - an explicit `-R/--repo/--repo=` wins; argument scanning stops at `--`;
  - otherwise the remote order is `remote.pushDefault` → branch
    `pushRemote` → branch `remote` → `origin` → the only remote;
  - strict GitHub.com HTTPS, `ssh://`, and scp forms only; owner
    `[A-Za-z0-9-]`, matched without regard to case;
  - a configured owner overrides ambient `GH_TOKEN`; an unconfigured owner
    keeps ambient behavior; a configured-but-empty token fails with exit 78
    and names the owner only;
  - the real `gh` is called by absolute path, so it cannot recurse.
- **`docs/knowledge-base/workspace-environment-inheritance.md`**: a workspace
  has several process boundaries. Local execution, cluster dispatch, local
  terminal, and worker terminal must all be audited. Resolve in one place,
  inject explicitly, and filter reserved names (`VK_*` is reserved, so org
  Env Vars cannot set it). Managed CLI login PTYs must keep their minimal
  environment. Its rule that *PATs never cross coordinator actions* came from
  the Nix design. Later, `b0d4` deliberately sends resolved org Env Var values
  through the authenticated worker environment transport. A UI-managed,
  coordinator-scoped setting has to use that same transport, and the plan
  must record this as a deliberate exception.
- **The same page, 1Password section**: use `services::environment_secrets`
  (`OP_SERVICE_ACCOUNT_TOKEN` from org Env Vars, or the service env; host-first
  `op` discovery; 30 s bound; secret-safe typed errors). Use
  `api_types::normalize_secret_reference` and web-core `secretReference.ts` as
  the single reference rule (strip copied quotes; case-sensitive `op://`). Show
  references and never return literals. Normalize drafts only by removing a
  quote pair while the user types, and fully at submit.
- **`wiki/managed-cli-tool-catalog.md`, "Workspace PATH propagation"**: derive
  app-owned directories on the host that spawns, **immediately before spawn**.
  Never send the coordinator's absolute app-data path to a worker. Keep
  workspace-only policy out of the generic PTY service; the terminal route
  applies it after it has chosen the remote-worker branch.
  `append_cli_tools_to_path` *appends* so that host tools win. A routing shim
  has to be *prepended* instead, or the real `gh` shadows it.

## Supporting

- **`homelab/docs/knowledge-base/vibe-kanban-executor-secret-environment.md`**:
  anything a worker needs must reach every eligible execution host. A change
  wired only on the coordinator passes local tests and then fails when a
  workspace is placed on a worker. New sessions pick up environment changes;
  running processes keep the environment they started with.
- **`docs/knowledge-base/clustered-workspace-execution.md`**: worker requests
  are signed over method, path, query, and body digest. The coordinator stays
  authoritative for configuration, and workers own spawning.
- **`crates/services/src/services/mcp_gateway_secrets.rs`**: host-bound
  AES-256-GCM envelope store (`load_or_generate(key_path)`, `encrypt/decrypt`
  with an AAD binding). The key file is under `utils::assets::asset_dir()`. It
  can be reused for encrypting local values at rest with its own key file.
- **Constitution**: principles 13 (actionable, secret-safe errors), 17
  (app-owned payloads, no silent provisioning), 25 (options must be honored at
  runtime), 45 (runtime-only credential values), 102 (capabilities proved at
  the execution boundary, with fresh-session proof distinct from static
  proof), and the `vk/b0d4` applicability note (encrypted at rest, fail closed
  on reference errors, reuse the resolver).

## Empirical check done for this task

With Git 2.54, owner-scoped credential contexts behave as follows when set
through `GIT_CONFIG_COUNT`:
`credential.https://github.com/<owner>.helper=` (reset) followed by an inline
`!f(){ … }` helper.

- They route only `https://github.com/<owner>/…`, including
  `user@github.com/<owner>/…` URLs.
- They leave `<owner>-other/…`, other owners, and host-only lookups on the
  existing helper.
- Path matching is **case-sensitive**: a `Sweetgreen/…` URL does not match a
  `sweetgreen` context.
