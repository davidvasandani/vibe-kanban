# Default every new workspace to the remote mainline

Task: `vk/1476-protect-git-repo`

## Problem

When Vibe Kanban starts a workspace, one repository-selection path defaults the
target branch to the registered checkout's current local branch. Repositories in
`/srv/src` may be checked out on deployment, recovery, or operator branches, so
that local state is not a safe workspace base. The intended default is the
remote mainline, normally `origin/main`.

## Required behavior

- Every new-workspace repository selection path uses the same default-branch
  policy.
- An explicitly configured repository default remains highest priority.
- Without an explicit default, `origin/main` is preferred, followed by
  `origin/master` for legacy repositories.
- Only when neither remote mainline exists may selection fall back to the
  current branch and then the first available branch.
- An explicit initial branch supplied by the calling workflow remains higher
  priority than repository/default inference when it exists.
- The exact selected remote-tracking branch is persisted as the workspace's
  target branch, so worktree creation resolves that ref rather than local HEAD.
- Empty branch lists remain non-selectable and existing manual overrides remain
  unchanged.

## Scope

This is a Vibe Kanban application change. It must not alter the checkout,
deployment, or branch configuration of any other service under `/srv/src`.

## Verification

- Unit coverage proves configured defaults and explicit initial branches win.
- Coverage proves `origin/main` and `origin/master` outrank a current local
  branch.
- Coverage proves the existing current/first fallback and empty-list behavior.
- Relevant frontend type, lint, format, and test checks pass.

## Follow-up: Durable MCP Screenshot Imports

### Objective

Render screenshots returned by MCP tools inline in Vibe Kanban's Codex chat
without dumping base64 or raw MCP content JSON into the tool result.

### Design

Use the existing executor log-normalization boundary. Base64 MCP `image` blocks
are decoded into the workspace's ignored `.vibe-attachments/` directory and
rendered as Markdown image references. Hosted MCP `resource_link` blocks whose
MIME type is `image/*` are downloaded immediately while their capability URL
is valid, persisted in `.vibe-attachments/`, and rendered using a
worktree-relative Markdown image reference. Conversation history must not
depend on the remote URL remaining reachable.

Apply the same normalization to both Codex protocol paths, including the direct
app-server item-completion path used by clustered Vibe Kanban workers.
The shared image node renders the resulting local attachment through the
existing worktree asset route.

### Security and Lifecycle

- Fetch only HTTP(S) resource links explicitly marked with an image MIME type.
- Bound remote fetch duration and response size; reject redirects so every
  destination is validated before a request is made.
- Import at most eight hosted images per result concurrently under one
  aggregate deadline, so per-image timeouts do not accumulate.
- Reject loopback, private, link-local, multicast, documentation, and
  unspecified destinations unless their exact origin appears in the
  deployment-managed `VIBE_MCP_IMAGE_ALLOWED_ORIGINS` allowlist.
- Also accept the exact origin in the existing deployment-controlled
  `FIRECRAWL_BROWSER_URL` when it is present in the Vibe process environment.
- Require the fetched response to remain an image before persistence.
- Verify a supported raster-image signature rather than trusting MIME headers;
  transient remote SVG is not imported.
- Keep base64 image persistence content-addressed and worktree-local.
- Treat the MCP URL as a transient transfer capability and do not retain it in
  normalized Markdown after a successful import.

### Firecrawl Browser Integration

The Firecrawl Browser service stores screenshots in its existing bounded
artifact store and returns a capability-bearing MCP `resource_link`. Screenshot
artifacts are reusable until their short TTL expires so both the inline
thumbnail and full-size preview can load them, including after the browser
session closes. Existing browser-download
artifacts remain single-use.

Hosted screenshots may expire according to Firecrawl's artifact policy after
Vibe Kanban imports them; chat rendering uses the durable local copy.

### Acceptance Criteria

1. Codex app-server MCP image results render as inline Markdown images.
2. Base64 image blocks continue to persist into `.vibe-attachments/`.
3. HTTP(S) `resource_link` blocks with `image/*` MIME types are copied into
   `.vibe-attachments/` before rendering.
4. Failed, oversized, non-image, and non-HTTP(S) resource links retain existing
   tool-result behavior and are not persisted.
5. Automated tests cover base64, successful hosted-image import, and rejected
   or failed links.
6. Web and desktop clients render the local attachment without depending on
   remote URL lifetime or reachability.
7. Firecrawl's `screenshot` tool returns a reusable, TTL-bound image
   `resource_link` without carrying base64 through MCP.

## Follow-up: MCP Refresh Nested Route

### Problem

The session MCP refresh and status handlers are nested below both workspace and
session path parameters, but each handler extracts only one UUID. Axum rejects
requests before the handler runs because the route contains two path arguments,
causing the refresh endpoint to return HTTP 500.

### Required behavior

- Both refresh endpoints extract the workspace and session UUID tuple expected
  by the nested route.
- The existing workspace extension remains the authority for the loaded
  workspace; the path workspace UUID is consumed only to satisfy route
  extraction.
- Refresh requests reach the deployment MCP refresh service instead of failing
  during Axum path extraction.
- No routes or services outside Vibe Kanban are changed.
