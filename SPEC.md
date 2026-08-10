# Technical Spec: Ship Firecrawl MCP Authentication to Cluster Workers

## Objective

Ensure Vibe Kanban worker-hosted coding agents receive both values required by the configured `firecrawl-browser` stdio MCP launcher:

- `FIRECRAWL_BROWSER_URL`
- `FIRECRAWL_BROWSER_AUTH_TOKEN`

This prevents remote workers such as think4 from reaching Firecrawl but failing `/api/internal/mcp-scope` bootstrap with HTTP 401.

## Design

Use the deployment module's generic `executorSecretRefs` worker option to resolve the Firecrawl bearer through the existing 1Password bootstrap path and export it into the long-running worker process. Executor subprocesses and their stdio MCP children inherit it.

The repository MCP definition sets the URL directly and allowlists `FIRECRAWL_BROWSER_AUTH_TOKEN` for forwarding from the worker environment. It does not use a literal `${VAR}` value: Codex's supported secret-forwarding mechanism for stdio MCP servers is `env_vars`.

The secret value must never enter the Nix store, repository, command line, logs, or generated agent configuration. Only the 1Password reference is declarative.

Configure each Vibe Kanban execution worker with the private Firecrawl URL and existing `op://Homelab/Firecrawl Browser MCP/bearer-token` reference.

When no user follow-up already exists, use:

- `homelab/modules/vibe-kanban-rebuild.nix`
- Vibe Kanban worker host declarations using that module
- Evaluation checks for paired configuration and rendered service environment

## Out of Scope

- Changes to the Firecrawl service itself.
- Changes to Firecrawl authentication policy or firewall rules.
- Embedding the bearer value in `.mcp.json`, Codex TOML, or the Nix store.

## Acceptance Criteria

1. Worker configuration declares the Firecrawl token reference through `executorSecretRefs`, while the MCP definition declares the service URL.
2. The worker service resolves and exports `FIRECRAWL_BROWSER_AUTH_TOKEN` before launching Vibe Kanban.
3. The Firecrawl stdio MCP definition allowlists `FIRECRAWL_BROWSER_AUTH_TOKEN` with `env_vars`, so Codex forwards it from the worker environment into the MCP child.
4. The 1Password bootstrap credentials are still removed before executor jobs start.
5. Existing generic worker-secret assertions validate the secret configuration.
6. The changed configuration is syntactically valid and independent Codex review reports no significant findings.

## Follow-up: Inline MCP Screenshot Results

### Objective

Render screenshots returned by MCP tools inline in Vibe Kanban's Codex chat
without dumping base64 or raw MCP content JSON into the tool result.

### Design

Use the existing executor log-normalization boundary. Base64 MCP `image` blocks
are decoded into the workspace's ignored `.vibe-attachments/` directory and
rendered as Markdown image references. Hosted MCP `resource_link` blocks whose
MIME type is `image/*` are rendered directly as Markdown images when their URI
uses HTTP or HTTPS.

Apply the same normalization to both Codex protocol paths, including the direct
app-server item-completion path used by clustered Vibe Kanban workers.
The shared image node recognizes HTTP(S) sources as previewable images, and the
desktop CSP permits HTTP(S) image loading without widening script or connection
permissions.

### Security and Lifecycle

- Do not fetch arbitrary resource links in the worker.
- Only render HTTP(S) resource links explicitly marked with an image MIME type.
- Keep base64 image persistence content-addressed and worktree-local.
- The MCP server remains responsible for authorizing and retaining hosted URLs.

### Out of Scope

- Changing the Firecrawl Browser service to host screenshots or emit
  `resource_link` results.
- Adding a Vibe Kanban artifact proxy or durable remote-image import.

### Acceptance Criteria

1. Codex app-server MCP image results render as inline Markdown images.
2. Base64 image blocks continue to persist into `.vibe-attachments/`.
3. HTTP(S) `resource_link` blocks with `image/*` MIME types render inline.
4. Non-image and non-HTTP(S) resource links retain existing JSON behavior.
5. Automated tests cover base64, hosted-image, and rejected-link behavior.
6. Web and desktop clients can load hosted HTTP(S) image sources.
