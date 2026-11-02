# Vibe Kanban

Vibe Kanban is a workspace for managing software projects with coding agents.[^lineage]
Plan work on a kanban board, give agents their own Git worktrees, follow their
progress, test the result, and review changes before merging. Run it on your
machine or host it centrally, with optional workers for distributing execution
across servers.

The app brings issues, conversations, terminals, previews, diffs and pull requests
into one place. It supports individual development and shared projects, with
configurable pipelines and durable follow-ups for work that spans several agent
turns.

## Working with Vibe Kanban

1. Create a project and organize its issues by status, priority, assignee and tags.
2. Start a workspace with one or more repositories, target branches and a coding agent.
3. Follow the conversation, answer questions, approve plans and send feedback.
4. Run the application in the preview, inspect changes and leave inline review comments.
5. Push a branch, open a pull request or merge locally, then complete the issue and archive the workspace.

Issues track the work; workspaces contain the checkout and execution history.
An issue can have multiple workspaces, and a workspace can have multiple agent
sessions. You can also start a workspace directly without creating an issue.

## Capabilities

### Planning and collaboration

- Kanban and list views with customizable project statuses, drag-and-drop ordering,
  priorities, assignees, tags, filters and sorting.
- Organizations and projects with member roles and invitations when the remote
  collaboration service is configured.
- Markdown descriptions, attachments, comments, sub-issues and issue relationships.
  Attachment uploads require configured storage.
- Linked workspaces and pull requests, with issue identity visible inside the
  workspace so you can follow a task across views.
- Comments that can stay as notes or explicitly deliver a reply to a linked agent
  session. Delivery state distinguishes a saved comment from an agent continuation.
- Global search across accessible organizations, projects, issues, workspaces and
  saved conversation prompts and final replies. Full session-log retrieval through
  MCP separately supports time, keyword, role and turn filters.

See [issue management](docs/issue-management.mdx),
[organizations](docs/cloud/organizations.mdx) and
[filtering](docs/cloud/filtering.mdx).

### Agent workspaces

- Separate Git worktrees and branches for concurrent tasks, including workspaces
  that span multiple repositories. Worktree isolation separates Git changes;
  execution permissions depend on the agent and host configuration.
- Multiple sessions, follow-up prompts, queued messages, image attachments, file
  mentions, slash commands and live tool output.
- Agent profiles with model and execution settings, plus controls for which agents
  and models appear in pickers. Plan mode, approvals, context usage and session
  forking depend on the executor's capabilities.
- Repository setup, development-server and cleanup scripts, integrated terminals,
  process logs, workspace notes and editor links, including Remote SSH.
- Pinning, archiving and workspace cleanup, with protection for active work and
  recovery information when execution is interrupted.
- A carousel for watching several workspace conversations together, responsive
  board and workspace layouts, collapsible panels and configurable notifications.

See [creating workspaces](docs/workspaces/creating-workspaces.mdx),
[multiple repositories](docs/workspaces/multi-repo-sessions.mdx),
[chat](docs/workspaces/chat-interface.mdx) and
[workspace management](docs/workspaces/managing-workspaces.mdx).

### Code review and Git

Review unified or side-by-side diffs, browse changed files, leave inline comments
and send the review back to the agent. GitHub review comments can appear alongside
local feedback.

The Git panel supports branch status, pushing, rebasing, conflict resolution and
pull-request creation. Multi-repository workspaces have per-repository actions,
plus **Merge All** and **Merge & Complete** for merging local changes and completing
the linked issue. Pull-request tools expose PR details, checks, updates and merge
actions. GitHub and Azure Repos integrations require their corresponding
credentials and CLI setup.

See [reviewing changes](docs/workspaces/changes.mdx),
[Git operations](docs/workspaces/git-operations.mdx),
[GitHub](docs/integrations/github-integration.mdx) and
[Azure Repos](docs/integrations/azure-repos-integration.mdx).

### Previews and browser work

Start a repository's development server and open it in the embedded preview.
The preview includes navigation, responsive and mobile sizes, component selection
as agent context, and built-in developer tools. Its in-app path is remembered
separately from the development server's address.

Configured deployments can also provide managed browser sessions shared between
people and agents. The workspace shows the browser while MCP tools navigate,
click, type, scroll, inspect pages and take screenshots. Explicit control ownership
prevents competing human and agent input. Optional preview leases give a remote
browser temporary access to one workspace development-server port.

See [browser testing](docs/browser-testing.mdx),
[shared browser control](wiki/browser-session-control-arbiter.md) and
[remote preview verification](docs/remote-browser-dev-server-verification.mdx).

### Pipelines and continuing work

Pipelines add ordered instructions to an issue and show stage progress in the UI.
The bundled choices include Basic, SpecKit, WikiLLM and parallel-agent workflows.
Edit pipeline definitions and choose defaults at machine, organization or project
scope. The SpecKit viewer lets you read and edit specifications, plans and task
checklists in the workspace.

Pipelines guide agents through a process; their stage markers do not replace test
results or pull-request checks.

For work that continues beyond the current turn:

- Background helpers keep supervised commands running outside the agent's process group.
- Pollers repeat a command on a bounded schedule, with a completion predicate or
  timeout. A completion predicate can trigger a follow-up prompt.
- Durable wakeups resume a session at a future time, or wait for an eligible turn
  boundary if it is busy. Timers survive service restarts.
- Tracked child issues run in independent workspaces and can resume the parent
  session when their turn settles. The parent reviews the result and integrates
  child commits explicitly.
- Optional automatic error remediation creates or reuses a repair issue and starts
  a configured pipeline, with limits to prevent recursive task creation.

See [pipeline definitions](assets/pipelines), [pipeline behavior](wiki/task-pipeline-block.md),
[pollers](wiki/vk-pollers.md), [wakeups](wiki/durable-session-wakeups.md) and
[tracked subtasks](wiki/tracked-background-subtasks.md).

### Integrations and tools

| Integration | What it provides |
| --- | --- |
| Vibe Kanban MCP server | Programmatic access to issues, repositories, workspaces, sessions, history, PRs, browser sessions and durable automation. Workspace-scoped mode gives agents their task context. |
| External MCP servers | Settings-managed server definitions and agent assignments, connectivity diagnostics and supported refresh or restart flows. |
| MCP OAuth | Optional OAuth access for external clients that cannot launch a local stdio server. Requires issuer and proxy configuration. |
| Jira | Optional bidirectional issue synchronization, project/status mapping and links back to Jira. |
| Slack | A message shortcut that creates an issue with its source link. Optional configured AI summarization can prefill the issue from the thread. |
| CLI tools and authentication | A pinned tool catalog, install/update/remove actions, host availability checks, vendor login flows and deployment-specific unattended reauthentication. |
| Organization environment | Shared execution variables, including `op://` references resolved through configured 1Password access. |
| GitHub owner credentials | Owner-specific tokens for repository and PR operations, with credential health reporting. |
| Managed skills | Browse skills and propose edits through pull requests when a skills repository is configured. |

Integrations need their own credentials and, in some cases, additional services.
Installing a tool does not authenticate it on every worker.

See [Vibe Kanban MCP](docs/integrations/vibe-kanban-mcp-server.mdx),
[MCP settings](docs/settings/mcp-servers.mdx),
[CLI tools](docs/settings/cli-tools.mdx),
[organization environment variables](docs/settings/organization-settings.mdx),
[Jira sync](wiki/external-connector-sync.md) and
[Slack setup](docs/integrations/slack-integration.mdx).

### Distributed execution and operations

An optional coordinator/worker deployment places workspaces on eligible servers
and keeps subsequent execution on the assigned worker. Settings expose worker
health, resource metrics, placement and recovery controls. Moving a workspace
uses a coordinated stop, reassignment and continuation flow, subject to executor
and artifact-transfer support.

Worker-owned agent turns, child tasks, pollers and development servers can keep
running through a coordinator restart. The coordinator reconnects to their output
and reconciles outcomes. Work running directly in the coordinator process has a
different restart lifecycle and may be interrupted. Worker failures are reported
as interrupted or indeterminate when completion cannot be established.

Paired remote hosts and relay access let you reach a running instance from another
device. These are separate from cluster workers and require their own setup.

See [clustered workers](docs/self-hosting/clustered-workers.mdx),
[restart behavior](wiki/coordinator-restart-handoff.md) and
[remote access](docs/remote-access.mdx).

## Supported coding agents

The executor registry includes:

| Agent | Setup guide |
| --- | --- |
| Claude Code | [Claude Code](docs/agents/claude-code.mdx), with optional [Claude Code Router](docs/agents/ccr.mdx) configuration |
| OpenAI Codex | [Codex](docs/agents/openai-codex.mdx) |
| Gemini CLI | [Gemini](docs/agents/gemini-cli.mdx) |
| Amp | [Amp](docs/agents/amp.mdx) |
| OpenCode | [OpenCode](docs/agents/opencode.mdx) |
| Cursor Agent | [Cursor](docs/agents/cursor-cli.mdx) |
| Qwen Code | [Qwen Code](docs/agents/qwen-code.mdx) |
| GitHub Copilot | [Copilot](docs/agents/github-copilot.mdx) |
| Droid | [Droid](docs/agents/droid.mdx) |
| Grok CLI | [Grok](docs/agents/grok.mdx) |

Authenticate the agents you intend to use on the execution host. Available models,
permissions and tools depend on the selected agent, account and profile.

## Build and run

Build this repository to run its current feature set. The source build uses:

- Git, a native C/C++ build toolchain, `pkg-config`, OpenSSL development libraries,
  libclang on Linux; equivalent build tools on macOS.
- `zip` and `flock` on PATH. macOS users need to install `flock` separately.
- Rust through rustup, using the toolchain pinned in [rust-toolchain.toml](rust-toolchain.toml).
- Node.js 22.12+ or 24 and pnpm 10.13.1, pinned in [package.json](package.json).
- An authenticated coding agent and any tools required by your project.

```bash
git clone https://github.com/davidvasandani/vibe-kanban.git
cd vibe-kanban
pnpm install --frozen-lockfile
./local-build.sh
node npx-cli/bin/cli.js
```

The build produces the local web app and server, MCP, review, worker, remote API
and relay binaries, then prepares the local launcher under `npx-cli/`. The
launcher opens the app in your browser. Building the binaries does not start
PostgreSQL, ElectricSQL or the remote collaboration stack.

`local-build.sh` currently embeds the hosted collaboration API as its default.
To use your own remote service, override it when launching:

```bash
VK_SHARED_API_BASE=https://kanban.example.com node npx-cli/bin/cli.js
```

Use the [self-hosting section](#self-hosting) to configure that service.

## Architecture

| Component | Responsibility |
| --- | --- |
| `packages/local-web` | Browser app served by the local execution server. |
| `packages/remote-web` | Frontend for the remote collaboration deployment. |
| `packages/web-core`, `packages/ui` | Shared feature logic and UI components. |
| `crates/server`, `crates/services`, `crates/local-deployment` | Axum API, workspace lifecycle, orchestration, terminals and previews. |
| `crates/db` | SQLite-backed local workspace, session and execution state. |
| `crates/executors` | Agent protocols, profiles, approvals and normalized output. |
| `crates/git`, `crates/git-host` | Git operations and hosting-provider integration. |
| `crates/remote` | Collaboration API, authentication, organizations and issues in PostgreSQL, with ElectricSQL synchronization and REST fallback reads. |
| `crates/worker` | Worker daemon for distributed execution. |
| `crates/mcp`, `crates/review` | MCP server and review CLI. |
| `crates/relay-tunnel`, `crates/relay-*` | Remote host access and relay transport. |
| `crates/tauri-app` | Optional desktop shell. |
| `shared` | Generated TypeScript types and schemas. |

The local execution server and remote collaboration API are separate components.
A local workspace can execute agents without operating a collaboration stack;
shared boards, organizations and integrations use the remote service.

## Self-hosting

For a persistent execution server, run the compiled binary under a service
manager. Start with [the example service](vibe-kanban.service.example) and
[environment file](vibe-kanban.env.example), adapting their paths and user to
your installation. Pin the backend address and use the matching MCP binary from
your build.

| Setting | Purpose |
| --- | --- |
| `HOST` | Bind address, default `127.0.0.1`. |
| `BACKEND_PORT` / `PORT` | Server port; `BACKEND_PORT` takes precedence. Otherwise the OS assigns a port. |
| `PREVIEW_PROXY_PORT` | Separate preview proxy port. Pin it for a persistent service. |
| `VIBE_BACKEND_URL` | Explicit backend URL for the MCP server. |
| `VIBE_KANBAN_MCP_COMMAND` | Path to this build's `vibe-kanban-mcp` executable. |
| `VK_ALLOWED_ORIGINS` | Comma-separated browser origins allowed to call the API behind a proxy or custom domain. |
| `VK_SHARED_API_BASE` | Remote collaboration API URL. |
| `VK_SHARED_RELAY_API_BASE`, `VK_TUNNEL` | Relay endpoint and opt-in tunnel mode. |

For example, a reverse-proxied site at `https://kanban.example.com` needs
`VK_ALLOWED_ORIGINS=https://kanban.example.com` on its execution server.

To operate the collaboration service, configure PostgreSQL, ElectricSQL, a JWT
secret and authentication. It supports GitHub/Google OAuth, bootstrap local
email/password authentication, or `VIBEKANBAN_SINGLE_USER_MODE=true`. Single-user
mode skips the normal login boundary and must sit behind your own access control.
Issue attachments need separately configured storage.

Use [the Docker deployment guide](docs/self-hosting/deploy-docker.mdx) and
[Compose configuration](crates/remote/docker-compose.yml) as starting points,
with this repository as the source checkout. Some inherited guides still show
upstream clone or package commands; use the build instructions above for this
repository's binaries.

Serve remote collaboration over HTTPS with HTTP/2 to avoid browser connection
limits affecting concurrent sync requests. A reverse proxy can terminate TLS;
the remote binary also accepts `TLS_CERT_PATH` and `TLS_KEY_PATH` for direct TLS.
See [attachment storage](docs/self-hosting/attachments-storage.mdx) and
[cluster setup](docs/self-hosting/clustered-workers.mdx) for those optional parts.

### Managed deployment

This repository's main-branch pushes trigger
[the homelab deployment workflow](.github/workflows/trigger-homelab-deploy.yml).
It sends the commit SHA to `davidvasandani/homelab`, where
[`modules/vibe-kanban-rebuild.nix`](https://github.com/davidvasandani/homelab/blob/main/modules/vibe-kanban-rebuild.nix)
owns the NixOS service build, hosting, health gates and rollback.

With `VK_RELEASES_DIR` set at build time, `local-build.sh` publishes versioned
releases with `current` and `previous` links. Services run from those artifacts,
not from a mutable source checkout. See [deployment details](wiki/self-hosted-deployment.md).

## Development

Install dependencies once in every fresh worktree before running repository
verification. For the watch-based development server, also install `cargo-watch`:

```bash
pnpm install --frozen-lockfile
cargo install cargo-watch
pnpm run dev
```

The dev script selects ports and seeds the local development database. Set
`VK_SHARED_API_BASE` before `pnpm run dev` if you want to connect it to a remote
collaboration service. To run that stack locally, configure `crates/remote/.env.remote`
using [the local development guide](docs/self-hosting/local-development.mdx), then
use `pnpm run remote:dev:persist` to retain its database on shutdown.
`remote:dev` removes Compose volumes when it stops.

| Command | Purpose |
| --- | --- |
| `pnpm run format` | Format backend workspaces and web packages. |
| `pnpm run check` | Frontend type checks and backend checks, including the remote crate. |
| `pnpm run lint` | Frontend linting and backend Clippy checks. |
| `cargo test --workspace` | Rust workspace tests. |
| `cargo test --manifest-path crates/remote/Cargo.toml` | Tests for the separately managed remote crate. |
| `pnpm --filter @vibe/local-web run build` | Build the local frontend. |
| `pnpm run generate-types` / `pnpm run remote:generate-types` | Regenerate shared local or remote TypeScript types. |
| `pnpm run prepare-db` / `pnpm run remote:prepare-db` | Prepare SQLx metadata; requires the relevant database tooling. |

Full-workspace checks include the optional Tauri crate and need its native
platform dependencies. The default `local-build.sh` selects the headless binaries;
desktop development is separate through `tauri:dev` and `tauri:build`.

See [AGENTS.md](AGENTS.md) for repository conventions. Edit Rust type definitions
and regenerate shared types rather than editing generated files.

## Documentation and contributing

- [Workspace guide](docs/workspaces/index.mdx) and [settings](docs/settings/index.mdx)
- [User documentation source](docs/README.md)
- [Project wiki](wiki/INDEX.md) and [engineering knowledge base](docs/knowledge-base/INDEX.md)
- [Issues and feature requests](https://github.com/davidvasandani/vibe-kanban/issues)
- [Pull requests](https://github.com/davidvasandani/vibe-kanban/pulls)

For contributions, describe the behavior being changed, keep the scope focused
and include relevant validation. The project is licensed under [Apache 2.0](LICENSE).

[^lineage]: This repository is a fork of [BloopAI/vibe-kanban](https://github.com/BloopAI/vibe-kanban), incorporating work from [2001zhaozhao/vibe-kanban](https://github.com/2001zhaozhao/vibe-kanban) and subsequent development here. See [CONTRIBUTORS.md](CONTRIBUTORS.md) and Git history for attribution.
