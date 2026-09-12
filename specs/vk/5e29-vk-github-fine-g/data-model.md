# Data Model: GitHub PAT Routing

There is no database schema or API entity.

## Deployment configuration

`GithubAuthConfig` (Nix submodule):

| Field | Type | Constraint |
| --- | --- | --- |
| `orgTokenRefs` | attrs of string | key is unique case-insensitive GitHub owner; value begins `op://` |
| `opTokenPath` | null or absolute runtime string | required when mappings exist unless Connect bootstrap is configured; never `/nix/store` |
| `opConnectHost` | null or string | optional Connect endpoint |
| `opConnectTokenPath` | null or absolute runtime string | required with Connect host; never `/nix/store` |

## Runtime representation

For each normalized owner, one file exists under the credential preparation
unit's runtime directory. The filename is deterministic and contains only a
validated lower-case owner. The file body is the PAT with its trailing newline
removed by the wrapper. Directory mode is `0700`; token file mode is `0400`.

The generated routing table maps normalized owner to deterministic filename.
It contains no PAT and no 1Password reference.

## Lifetime

The runtime directory is created and removed by systemd. The oneshot populates
it before the Vibe Kanban execution service starts. Rotation is applied by
restarting the oneshot and dependent execution service. Nothing is persisted to
the database, Nix store, home-directory CLI config, or workspace.
