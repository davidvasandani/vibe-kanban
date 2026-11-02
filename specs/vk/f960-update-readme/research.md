# Feature and documentation audit

Task: f960-update-readme. Examined the checked-out app at 906d62d4, existing README, both knowledge indexes, user docs, routes, executor registry, settings components, build scripts and the Vibe Kanban homelab module. This is a product inventory, not a promise of capability parity across executors or deployments.

| Capability | Evidence | README treatment |
| --- | --- | --- |
| Issues, organizations, projects, statuses, assignees, tags, sub-issues | docs/issue-management.mdx; docs/cloud; crates/remote/src/routes | Planning workflow; remote collaboration prerequisite |
| Multiple repositories, isolated worktrees, sessions, setup/dev/cleanup scripts | docs/workspaces; crates/server/src/routes/workspaces; crates/git | Core workspace workflow; worktrees are Git separation, not a security sandbox |
| Agent registry and profiles | crates/executors/src/executors/mod.rs; docs/agents; settings/AgentsSettingsSection.tsx | Ten registered production executor types; CCR is Claude routing configuration, not a separate enum variant; avoid exact provider model versions |
| Chat, approvals, context, files/images, follow-ups | docs/workspaces/chat-interface.mdx; packages/web-core/src/pages/workspaces | Agent interaction; qualify provider-dependent controls |
| Diffs, PRs, rebases, multi-repo merge/completion | docs/workspaces/changes.mdx and git-operations.mdx; GitPanelContainer.tsx; crates/mcp/src/task_server/tools/pull_requests.rs | Core review plus Merge All and Merge & Complete; distinguish local merge from PR merge |
| Preview and managed shared browser | docs/browser-testing.mdx; BrowserPanelContainer.tsx; crates/mcp/src/task_server/tools/browser.rs | Separate app preview from configured browser sessions; preview leases are optional infrastructure |
| Pipeline editor and SpecKit artifacts | assets/pipelines; crates/server/src/routes/pipelines.rs and speckit.rs; wiki/task-pipeline-block.md | Prompt stages, defaults and artifact viewer; not a CI guarantee |
| Durable helpers, pollers, wakeups and child tasks | crates/mcp/src/task_server/tools/{background_helpers,pollers,sessions,workspaces}.rs; wiki/{vk-pollers,durable-session-wakeups,tracked-background-subtasks}.md | Explain persistence/continuation; child checkout needs explicit commit integration |
| Issue comments routed to sessions | wiki/issue-comment-delivery.md; crates/server/src/routes/issue_comment_deliveries.rs | Explicit agent reply versus ordinary note |
| Global and full session search | wiki/global-search.md; wiki/session-chat-history-search.md; routes/global_search.rs | Global search covers saved prompts/final replies, not every tool event; MCP session history is separate |
| Jira and Slack | crates/remote/src/jira; routes/jira_sync.rs and slack.rs; docs/integrations/slack-integration.mdx; wiki/slack-shortcut-ai-summarization.md | Optional bidirectional Jira and Slack shortcut with optional configured AI summaries |
| MCP integrations and inbound OAuth | crates/mcp/AGENTS.md; docs/settings/mcp-servers.mdx; wiki/embedded-mcp-oauth-server.md | Distinguish tools agents consume from tools VK exposes; use bundled build override |
| CLI management, AWS, auth, environment, GitHub owners, skills | settings/{CliTools,Aws,Auth,Organizations,Skills}SettingsSection.tsx; routes/{cli_tools,reauth,github_owner_tokens,skills}.rs | Host-dependent tools/auth; op:// references; skills proposals require configured repository |
| Metrics, workers, affinity and recovery | docs/self-hosting/clustered-workers.mdx; wiki/coordinator-restart-handoff.md; settings/WorkersSettingsSection.tsx | Optional coordinator/worker setup; worker-owned work survives coordinator restart, local work does not |
| Automatic error remediation | wiki/auto-error-remediation.md; settings/AutoErrorRemediationSettingsCard.tsx | Opt-in bounded remediation, not universal automatic repair |
| Responsive boards, carousel, settings and notifications | wiki/{workspace-carousel-view,settings-drawer,mobile-kanban-scrolling}.md; docs/settings/general.mdx | Usability summary without historical bug changelog |
| Deployment | local-build.sh; .github/workflows/trigger-homelab-deploy.yml; ../homelab/modules/vibe-kanban-rebuild.nix | NixOS managed deployment, dispatch on main, immutable releases and health gating; remove old Ubuntu/private-image claims |

## Setup corrections and boundaries

- rust-toolchain.toml pins nightly-2025-12-04; package.json pins pnpm 10.13.1. CI uses Node 22. Recommend Node 22.12+ or 24 rather than the old broad Node >=20 statement with Vite 7.
- local-build.sh builds the local web, server, MCP, review, worker, remote and relay binaries plus the launcher. It installs npx-cli dependencies but needs root pnpm dependencies first. Headless build excludes Tauri; --desktop is optional with native prerequisites.
- local-build.sh explicitly sets the hosted collaboration API at build time. Runtime VK_SHARED_API_BASE overrides it in crates/local-deployment/src/lib.rs. Explain this near the build command, so source build does not imply a full self-hosted collaboration stack.
- BACKEND_PORT takes precedence over PORT. PREVIEW_PROXY_PORT is separate. VIBE_BACKEND_URL pins MCP backend routing; VIBE_KANBAN_MCP_COMMAND selects this build's MCP binary.
- crates/remote/docker-compose.yml is a development baseline. remote:dev uses down -v; recommend remote:dev:persist for retained data. The relay is an optional profile, despite older local-development prose saying it always starts.
- Remote config supports OAuth, bootstrap local credentials and VIBEKANBAN_SINGLE_USER_MODE. Single-user mode needs access control at the network/proxy boundary. TLS_CERT_PATH/TLS_KEY_PATH enable direct remote TLS, or a proxy can provide HTTPS/HTTP2.
- Some inherited guides still use upstream URLs and launch commands. README instructions explicitly select this checkout; detailed guides remain references, not evidence of an identical distribution.

## Editorial decisions

Use one product narrative instead of separate original/addition lists. Retain ancestry and contributor links in a footnote. Remove inherited personal commentary, hiring/promotional badges, old deployment narrative and aspirational swarm goals. Prefer workflow links over a historical bug-fix list. Screenshots already exist but are historical, so omit them rather than imply they show the current responsive UI.
