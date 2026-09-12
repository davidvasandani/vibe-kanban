# Technical Plan: GitHub PAT Routing by Repository Owner

**Spec**: `./spec.md`  
**Status**: Ready for tasks

## Technical Context

The implementation is deployment-owned Nix and POSIX shell in
`homelab/modules/vibe-kanban-rebuild.nix`, with Nix evaluation and derivation
tests in `homelab/tests/vibe-kanban-cluster.nix`. No Vibe Kanban application API,
database, Rust type, or frontend change is required.

Vibe Kanban coordinator processes run as `vibe-kanban-dev`; workers run as
`vibe-kanban`. Both systemd units construct their executable search path from
the NixOS `path` option, which is inherited by agents, lifecycle scripts, dev
servers, and PTYs. Installing a `gh` routing wrapper at the front of those unit
paths covers every workspace process boundary without placing secrets in the
application environment or cluster protocol.

## Architecture & Approach

### Module options

Add a `githubAuth` submodule under `services.vibe-kanban-rebuild`:

- `orgTokenRefs`: attribute set from canonical owner name to 1Password ref;
- `opTokenPath`: runtime credential path, defaulting to the worker's existing
  bootstrap path for worker role and `/var/lib/developer/op-token` otherwise;
- `opConnectHost` and `opConnectTokenPath`: optional Connect routing matching
  the worker credential bootstrap contract.

Assertions reject invalid owner names, case-insensitive duplicates, refs that
do not start with `op://`, missing bootstrap paths when mappings exist, relative
credential paths, and `/nix/store` paths.

### Credential preparation

When `orgTokenRefs` is non-empty, add a oneshot
`vibe-kanban-github-tokens.service`. It runs as the same user/group as the local
execution owner (`vibe-kanban-dev` coordinator; `vibe-kanban` worker), receives
the 1Password bootstrap via `LoadCredential`, and uses a systemd
`RuntimeDirectory` with mode `0700`.

The service resolves each ref to a temporary file, rejects empty values, sets
mode `0400`, and atomically renames it to a filename derived from the normalized
owner. Failure leaves the main execution service stopped rather than silently
falling back for a configured owner. PATs never appear in arguments, generated
Nix text, logs, or the shared workspace. The unit is ordered before and required
by `vibe-kanban-dev` or `vibe-kanban-worker`.

### Router package

Generate a package containing `bin/gh` plus an internal resolver script. The
wrapper uses absolute store paths for `git`, `coreutils`, and real `gh`, so it
does not recurse and does not depend on a workspace's PATH.

The resolver:

1. scans arguments for `-R value`, `--repo value`, and `--repo=value`;
2. otherwise selects a Git remote according to clarification C2;
3. accepts `OWNER/REPO`, `https://github.com/OWNER/REPO(.git)`,
   `ssh://git@github.com/OWNER/REPO(.git)`, and
   `git@github.com:OWNER/REPO(.git)`;
4. strictly rejects other hosts and malformed owners;
5. lowercases the owner and dispatches through a generated `case` table;
6. reads the matching runtime file, rejects missing/empty values, exports
   `GH_TOKEN`, and execs the real GitHub CLI; or
7. execs real `gh` with the caller environment unchanged if no owner matches.

The case table contains only normalized owner names and runtime filenames. It
contains no PAT or 1Password reference.

### Unit integration

Prepend the router package to `systemd.services.vibe-kanban-dev.path` for the
coordinator and `systemd.services.vibe-kanban-worker.path` for a worker. Add the
credential unit dependency only when routing is configured. Because worker
dispatch serializes actions rather than the systemd unit environment, no
protocol or payload change occurs; the worker resolves and reads its own
node-local credentials.

Leave the owner map empty by default. Concrete 1Password references were not
part of the request, and inventing them would prevent the fleet from starting.
The operator documentation gives the exact coordinator/worker activation shape;
nodes can opt in once their real references are known.

## Data Model

See `./data-model.md`. There is no persistent application data model.

## Contracts

See `./contracts.md`. The contract is the Nix option shape and command-routing
behavior; there is no HTTP or cluster-protocol change.

## Research Notes

See `./research.md`. No new dependency is introduced.

## Constitution Check

- **I / III / VI:** deployment-level PATH wrapping is smaller than adding a
  database/API credential system and reuses current systemd execution paths.
- **II:** pure wrapper behavior gets a fake-`gh` derivation test; option and unit
  wiring get Nix evaluation assertions.
- **VIII:** the real `gh` is the pinned nixpkgs package and configuration remains
  operator/host-managed.
- **XVIII:** credentials are independently provisioned on the assigned worker;
  affinity and dispatch semantics are unchanged.
- **XX:** runtime credential paths are node-local and never written to shared
  storage.
- **XXI:** one wrapper owns target parsing and errors identify owner/configuration
  without including secret values.
- **XXII:** owner selection occurs at each `gh` invocation; the long-lived server
  inherits only PATH, not PAT values, and cluster actions remain secret-free.

No constitution deviation or open question remains.

## Risks & Dependencies

- NixOS systemd path merging must keep the router before system `gh`; evaluation
  tests inspect the rendered unit path.
- `ProtectSystem=strict` permits reads from `/run`; the token unit owns its
  runtime directory with the same execution identity.
- A token rotation requires restarting the preparation and Vibe Kanban execution
  units; documentation provides the exact workflow.
- GitHub CLI may add new global repository argument forms. The supported forms
  are explicit and covered by tests; unknown forms fall back unchanged.
- 1Password must be reachable during unit start. Failure is deliberate and
  visible because silently using another PAT violates least privilege.

## Verification

1. Evaluate `homelab/tests/vibe-kanban-cluster.nix`.
2. Build/run the wrapper derivation tests with fake credentials and fake `gh`.
3. Run `nixfmt --check` (or repository formatter) on changed Nix files.
4. Inspect rendered coordinator and worker units for router precedence,
   credential dependencies, and absence of token/ref contents in wrapper text.
5. Search the diff for token-shaped values and confirm no cluster protocol type
   changed.
6. Run independent Codex review and re-run checks after fixes.
