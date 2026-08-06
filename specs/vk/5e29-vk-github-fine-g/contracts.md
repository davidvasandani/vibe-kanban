# Contracts: GitHub PAT Routing

## Nix configuration

```nix
services.vibe-kanban-rebuild.githubAuth = {
  opTokenPath = "/var/lib/developer/op-token";
  orgTokenRefs = {
    BloopAI = "op://Homelab/GitHub PATs/BloopAI";
    davidvasandani = "op://Homelab/GitHub PATs/davidvasandani";
  };
};
```

Worker hosts may additionally set `opConnectHost` and
`opConnectTokenPath`. The example values are references, not secrets.

## Command routing

| Context | Result |
| --- | --- |
| `gh -R Owner/repo ...` and Owner configured | Owner PAT overrides ambient `GH_TOKEN` |
| `gh --repo=Owner/repo ...` and Owner configured | same |
| no explicit target; selected remote is GitHub.com/Owner/repo | Owner PAT |
| owner parsed but not configured | exec real `gh` unchanged |
| no repo or non-GitHub remote | exec real `gh` unchanged |
| configured owner token missing/empty | non-zero local error naming owner, no real `gh` execution |

Explicit target selection takes priority over the current directory.

## Secret boundary

Inputs to cluster-dispatched execution remain unchanged. The coordinator action
contains no owner map, token ref, credential path, or PAT. Each execution node
gets the wrapper and runtime files from its own NixOS unit configuration.
