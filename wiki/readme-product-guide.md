# README product guide and evidence boundaries

The root README describes the current product by workflow. Ancestry belongs in
its footnote, with licensing and contributor history preserved. Do not restore
historical fork commentary, upstream hiring badges or obsolete hosting notes
when updating it from another branch.

## Verify distribution separately from capability

A source checkout is not the same thing as an installed distribution.
`local-build.sh` builds this repository's binaries and launcher, but explicitly
embeds the hosted collaboration API as the default. Runtime `VK_SHARED_API_BASE`
overrides that default in `crates/local-deployment/src/lib.rs`. A build alone does
not provision PostgreSQL, ElectricSQL or collaboration authentication.

The MCP command is another distribution boundary: `VIBE_KANBAN_MCP_COMMAND` must
select the matching built executable for a persistent deployment. Otherwise
catalog or launcher defaults can resolve a public package with different tools.
Read `vibe-kanban.env.example` before documenting server/MCP startup.

The remote Compose scripts have materially different data lifetimes:
`remote:dev` runs `down -v`, while `remote:dev:persist` retains volumes. Recommend
the persistent variant when the reader intends to keep their board data.

## Keep operational claims scoped

- Local SQLite owns execution state; remote PostgreSQL owns collaboration issues.
- Workers and paired remote hosts are different systems. Worker-owned processes
  survive coordinator restarts; directly coordinator-owned executions do not
  have that same guarantee.
- Global search indexes saved user prompts and final assistant replies. MCP
  session-history retrieval separately covers intermediate messages and tools.
- Pipelines are instructions given to agents. A stage marker is not evidence of
  successful tests or a replacement for CI.
- Tool installation, authentication and execution-host availability are separate
  facts. Do not imply optional Jira, Slack, browser, 1Password or skills services
  are enabled just because their implementation ships in the repository.

The Vibe Kanban hosting contract is in homelab's
`modules/vibe-kanban-rebuild.nix`; main-branch dispatch is in
`.github/workflows/trigger-homelab-deploy.yml`. Avoid copying an old private-image
or Ubuntu deployment story into the product guide.

## Sources and maintenance

Both `wiki/INDEX.md` and `docs/knowledge-base/INDEX.md` contain maintained project
knowledge. Source and scripts settle conflicts with inherited documentation.
Some detailed guides still contain upstream clone/package examples; the root
README's source instructions identify the intended checkout explicitly.

For a feature overview refresh, recheck executor registration, settings sections,
MCP tools, remote integrations, build scripts and CI, then validate relative
links and script names. Keep exact model inventories in the existing executor
guides instead of duplicating them in the README.

## Contributed by

- vk/f960-update-readme
