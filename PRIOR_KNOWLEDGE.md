# Prior Knowledge: Gmail MCP connector (`vk/4daf-gmail-mcp`)

The project knowledge base is populated. This repository carries **two**
knowledge bases and both were searched:

- `docs/knowledge-base/` (22 pages, `INDEX.md`) — the current one; task ids match
  `specs/vk/*`. Every MCP page lives here.
- `wiki/` (19 pages, `INDEX.md`) — an earlier generation, still authoritative for
  frontend, lifecycle and catalog-pinning topics.

This task adds a bundled Gmail MCP catalog entry sourced from a fork, plus
generic multi-instance template instantiation. The MCP-catalog pages are directly
on-topic; the rest supply pinning, secret-hygiene and frontend-state rules.

## Most relevant pages

| Page | Why |
| --- | --- |
| `docs/knowledge-base/shared-mcp-configuration.md` | The catalog contract itself: transport-neutral entries, placeholders not secrets, the Opencode `environment` rename, `meta.url` is a link not a build instruction |
| `docs/knowledge-base/forked-mcp-server-packaging.md` | The whole fork-pinning idiom — delivery choice, mutable-pin failure mode, shape-test-not-string-test, Renovate traps, cache-isolated verification |
| `wiki/managed-cli-tool-catalog.md` | The parallel pinning discipline for the CLI catalog: stable wire ids, immutable version-addressed URLs, SHA-256 pins, generated types |
| `docs/knowledge-base/mcp-connectivity-testing.md` | "VK is a config writer, not an MCP client"; per-agent entry adaptation; stdio probe handshake used for manual verification |
| `docs/knowledge-base/mcp-oauth-connect.md` | Why the encrypted shared gateway exists and what it covers — the boundary that excludes a stdio server like Gmail |
| `docs/knowledge-base/active-mcp-refresh.md` | What happens to a live session when MCP config changes; sets expectations for adding a server mid-flight |
| `docs/knowledge-base/workspace-environment-inheritance.md` | Never log or debug-format resolved secrets |
| `wiki/slack-shortcut-ai-summarization.md` | Write-only encrypted key handling and no-secrets-in-logs hygiene for the other Slack surface |
| `docs/knowledge-base/executor-model-catalog-maintenance.md` | Keeping a bundled catalog and its documentation moving together |
| `wiki/bundled-file-seed-manifests.md` | How bundled defaults reach existing installs — relevant because a catalog addition must not disturb saved native config |
| `docs/knowledge-base/worktree-formatting-prerequisites.md` | Fresh-worktree setup and the verification sequence |
| `wiki/project-context-map.md` | CI check false-pass traps worth knowing when adding an assertion |

## Hard constraints extracted for this task

### The catalog contract

1. **`crates/executors/default_mcp.json` is canonical, and entries stay
   transport-neutral.** "Keep entries transport-neutral in that file (`command`,
   `args`, and `env` for stdio), use credential placeholders rather than secrets,
   and let `mcp_config.rs` adapt the entry to each executor's native schema."
   *(shared-mcp-configuration)* — Gmail is plain stdio `command`/`args`/`env`;
   no per-executor branch is added.

2. **Opencode renames the stdio environment field.** "Opencode calls the stdio
   environment field `environment`; dropping or leaving it as `env` makes
   credential-dependent catalog entries unusable after adaptation."
   *(shared-mcp-configuration)* — Gmail carries an `env`, so it needs the same
   Codex/Opencode adaptation test the Slack entry has.

3. **`meta.<server>.url` is a link, not a build instruction.** "A Slack entry can
   advertise `github.com/davidvasandani/slack-mcp-server` while
   `npx -y slack-mcp-server@latest` installs the upstream package — the UI claims
   one repository, the machine runs another… Two independent defects hide in one
   line: wrong source, and a mutable pin. Assert both, or neither is
   guaranteed." *(forked-mcp-server-packaging)* — the Gmail test must assert that
   `meta.gmail.url`'s owner/repo equals the install spec's owner/repo.

4. **A shape test beats a string test.** "Parse the URL into
   `owner/repo/tag/asset`, assert `owner/repo` equals the owner/repo in
   `meta.<server>.url`, assert `tag` equals a named constant, and reject
   `@latest`, `#master`, `refs/heads/`, `/archive/`. That test fails for the
   *next* person who reaches for a mutable pin, not just for today's."
   *(forked-mcp-server-packaging)* — adopted verbatim, with `tag` replaced by a
   40-hex commit-ish and `#main` added to the reject list.

5. **Prefer immutable, version-addressed sources.** "Pin every downloadable
   artifact with a SHA-256 digest. Prefer immutable, version-addressed vendor
   URLs; a version bump must include refreshed hashes." *(managed-cli-tool-catalog)*
   — a git commit SHA satisfies the immutability requirement directly; the
   separate digest constant exists only because a release asset is mutable under
   a fixed tag, which a commit SHA is not.

6. **Catalog changes do not rewrite already-saved native entries.** "Catalog
   changes do not rewrite native executor files that were saved from an older
   bundled template." *(shared-mcp-configuration)* — Gmail is new, so there is no
   historical template to migrate. `canonical_definition_for_server` is hard-keyed
   on the literal name `"slack"`; that is precedent **not** to copy.

### Fork delivery

7. **The `npx github:owner/repo#<sha>` rejection was Slack-specific.** The
   rejected-alternatives table reads: "Pinned, but clones the whole (Go) repo per
   cache miss and still needs a binary source at run time."
   *(forked-mcp-server-packaging)* — both halves are properties of a Go fork. A
   TypeScript package with `"prepare": "npm run build"` is runnable straight from
   a checkout, so the objection does not transfer. The spec records the divergence
   explicitly rather than silently contradicting the page.

8. **The preferred end state is a fork-controlled npm package at an exact
   version.** "npm would verify the packument's `dist.integrity` before the
   package's `bin` runs… Move the catalogue entry to an exact registry version,
   confirm `dist.integrity`, switch Renovate to the npm source, and update the
   source constant, tests, and both documentation layers in the same reviewed
   change." *(forked-mcp-server-packaging)* — recorded as the documented fallback
   if cold-start build time proves painful.

9. **A digest nothing re-checks is a comment, not a control.** "A digest that
   nothing re-checks on a schedule is a comment, not a control."
   *(forked-mcp-server-packaging)* — the reason this task adds **no** Gmail digest
   constant: an unaudited constant would be worse than none, and the commit SHA is
   enforced by npm at install time on every machine, which is the stronger layer.

10. **Renovate coverage that looks real and is not.** "Renovate needs
    `ignoreUnstable: false` for fork tags… the default stability filter makes the
    manager match the pin and then never propose anything — coverage that looks
    real and is not." *(forked-mcp-server-packaging)* — a bare commit SHA on a
    release-less fork is untrackable, so the pin is documented as manually bumped
    instead of wired to a manager that would silently never fire.

11. **Verify with a cold cache.** "Cache isolation is the whole point of the
    exercise: run with a fresh `npm_config_cache`… a warm cache will happily prove
    the previous artifact works. Then drive the server over stdio (`initialize` →
    `notifications/initialized` → `tools/list`)."
    *(forked-mcp-server-packaging)* — this is the manual verification procedure.

12. **Scope pre-existing Renovate `packageRules` with `matchFileNames` when
    adding a second dep to the same datasource, or the older rule's
    `prBodyNotes` will give confidently wrong instructions about the new one.**
    *(forked-mcp-server-packaging)* — applies if a Gmail manager is ever added.

### Secrets and the gateway boundary

13. **Vibe Kanban is an MCP config *writer*, not a client.** "The MCP Servers
    settings screen writes server entries into each coding agent's own config
    file… Nothing in VK ever *connected* to those servers."
    *(mcp-connectivity-testing)* — anything typed into a catalog entry's `env`
    lands in plaintext in each assigned agent's global config. Gmail's entry
    carries a credentials **path**, not a token, which keeps the refresh token
    under the Gmail server's own ownership.

14. **The encrypted shared gateway is HTTP-only.** "OAuth-capable streamable HTTP
    assignments can use the local Vibe MCP gateway… Upstream access and refresh
    tokens are encrypted in SQLite with a host-local AES-GCM key and never enter
    agent configuration files." *(shared-mcp-configuration)* — a stdio server
    cannot use it, which is why in-app Google OAuth is a rejected alternative
    rather than an oversight.

15. **Placeholders are unvalidated.** There is no placeholder-handling code
    anywhere; `YOUR_TOKEN` / `YOUR_API_KEY` are literal strings that flow
    untouched into the native config file if never edited. No masking, no secret
    input type. The docs therefore carry the "you must edit this" weight.

16. **Never log or debug-format resolved secrets.**
    *(workspace-environment-inheritance)* — no new logging is added here, but it
    constrains any diagnostic work on the entry.

### Identifiers and frontend state

17. **MCP server names are protocol identifiers, not display labels.**
    `is_valid_server_identifier` enforces `^[a-zA-Z0-9_-]+$`
    (`crates/executors/src/shared_mcp_config.rs:208`), duplicates are rejected in
    `validate_write_request` (`:928`), and `suggested_server_identifier` exists to
    push users to the snake_case form. Auto-generated instance names must be valid
    by construction, so the suffix is `_2`, not ` (2)`.

18. **There is no display-label field.** `SharedMcpServer` carries only `name`
    (`shared_mcp_config.rs:87`), and native agent configs store nothing but the
    map key — a human-readable label would have nowhere to persist without
    introducing the first VK-owned MCP store. "Gmail MCP (Personal)" can only be
    an identifier like `gmail_personal`.

19. **Same name + different credentials is a conflict, not two servers.** The
    reconciliation tests treat differing token values under one server name as a
    conflict (`shared_mcp_config.rs`, `equivalent_slack_conflicts_on_semantic_stdio_differences`).
    Per-account credentials therefore *require* per-account names — this is the
    mechanical reason multi-instance support is a prerequisite, not a nicety.

20. **The dialog owns provisional state and must re-seed on every open.**
    "NiceModal reuses mounted components, so every open must re-seed all editable
    fields and assignments." *(shared-mcp-configuration)* — relevant because the
    editor opens immediately after a template is added.

21. **Assignment compatibility follows the shared materialization contract, not
    an editor codec's narrower surface.** *(shared-mcp-configuration)* — Gmail is
    stdio, compatible everywhere, so this only means: do not add a special case.

### Process

22. **Documentation layers move together with the pin.** The Slack precedent
    couples `default_mcp.json`, the constants in `mcp_config.rs`, the docs page,
    the Renovate rule and `AGENTS.md`; `AGENTS.md` states the rule explicitly.
    *(forked-mcp-server-packaging, executor-model-catalog-maintenance)*

23. **Fresh worktrees need `pnpm install --frozen-lockfile` before verification,
    and `pnpm run format` before completion.** *(worktree-formatting-prerequisites,
    `AGENTS.md`)*

## Corrections this task should make to the knowledge base

`docs/knowledge-base/shared-mcp-configuration.md` states: *"The backend exposes
this catalog through `/api/mcp-config/default`, but the current shared MCP
settings UI does not render catalog suggestions. Treat catalog availability and
UI discoverability as separate capabilities when scoping work."*

That is now stale. `McpSettingsSection.tsx:1094-1161` renders the catalog as a
"Popular servers" tile grid driven by `preconfiguredMcpServers()`. The
knowledge-distillation stage should correct it rather than leave a page that
would mislead the next person into thinking a UI surface still needs building.

## Gaps — nothing in either knowledge base covers these

- Running **multiple instances of one catalog template**. No page discusses
  instance naming, tool-name collision across same-server instances, or
  per-instance credentials. This is the reusable knowledge this task produces.
- **Client-side tool-name dedupe across MCP servers** as a failure mode. The
  Gmail server's `--tool-prefix` exists solely for it, and nothing in the repo
  records that some clients dedupe tools by base name.
- Pinning a **git commit SHA** rather than a release asset or registry version,
  and the reasoning about which integrity layer that removes the need for.
# VAS-356 Addendum: Cluster MCP runtime connectivity

The knowledge bases are populated. Relevant pages were found in both the Vibe
Kanban repository and the shared homelab repository.

- `docs/knowledge-base/active-mcp-refresh.md`: Codex reload acknowledgement is
  only a queue acknowledgement; the next turn's `mcpServerStatus/list` is the
  authoritative inventory. Connectivity failures must remain explicit rather
  than being mistaken for persistence failures.
- `docs/knowledge-base/workspace-environment-inheritance.md`: values required by
  child processes must be injected at the execution boundary. Reserved `VK_*`
  process-owned variables take precedence, and diagnostics must never print the
  complete environment.
- `homelab/docs/knowledge/nftables-service-port-scoping.md`: think1 has its broad
  NixOS firewall disabled, so the dedicated nftables base chain is the active
  enforcement mechanism. Accepts must precede the targeted drop, and widening
  one port must not accidentally widen adjacent protected ports.
- `homelab/apps/firecrawl-browser-service/README.md`: port 3410 was intentionally
  restricted to think2 for the Cloudflare connector. VK workers are a new,
  explicitly authorized consumer and must be enumerated as source addresses.

Implementation consequence: derive both worker URL environment variables from
the same Nix option, and split the nftables rule so only 3410 gains the VK worker
sources while 8189/8190 retain their existing two-host allowlist.
