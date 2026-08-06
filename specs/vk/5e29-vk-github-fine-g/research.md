# Research: GitHub PAT Routing by Repository Owner

## Decision R1 — Route through a deployment-provided `gh` wrapper

GitHub CLI credentials are host-scoped; all GitHub.com organizations share one
host. A single `GH_TOKEN` or `hosts.yml` entry therefore cannot express an
owner-specific identity. Routing at command invocation is the narrowest point
where explicit `--repo` and current-directory repository context are both known.

Alternatives rejected:

- Workspace-wide `GH_TOKEN`: wrong for a multi-owner workspace.
- Rewrite `hosts.yml` before each command: races across concurrent workspaces
  and persists the last secret beyond the command.
- Store PATs in the Vibe Kanban remote database: expands product/API/UI scope
  and transmits secrets to workers when deployment-local credentials suffice.
- Agent prompt instructions/aliases: do not cover scripts, dev servers, or PTYs
  reliably and are easy to bypass.

## Decision R2 — Reuse unit PATH inheritance

Coordinator and worker services already own the PATH inherited by their child
processes. Prepending one wrapper package covers managed executions and PTYs
without changing every Rust spawn call. Non-workspace login PTYs are separate
services/paths and therefore remain unaffected.

## Decision R3 — Resolve 1Password refs in a prerequisite unit

Existing Vibe Kanban deployment modules load a 1Password service-account or
Connect token through systemd credentials, fetch execution credentials during
startup, and avoid passing the bootstrap to agents. A dedicated oneshot extends
that pattern to several PATs and stores them in `/run`, owned by the execution
user. Fetching on every `gh` call would expose/bootstrap 1Password access to
workspace processes and add network latency and availability to every command.

## Decision R4 — Fail closed only after a configured-owner match

Unknown/non-GitHub contexts preserve existing authentication. Once an owner is
configured, a missing/empty PAT is a deployment error: falling through to an
ambient token could perform an action as the wrong identity. The error names the
owner but never the path contents.

## Dependencies

No new dependency. The implementation uses nixpkgs `gh`, `git`, `bash`,
`coreutils`, and `_1password-cli`, all already present in the deployment.
