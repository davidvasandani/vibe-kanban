# AWS SSO across service hosts and agent homes

Settings AWS authentication checks run on the selected Settings host. They do
not prove that a workspace worker can access either that host's AWS state or
its executable. CLI Tools has the same reporting boundary; label its evidence
as host-scoped unless worker reachability has actually been checked.

## Managed cluster deployment contract

The homelab Vibe Kanban rebuild module provisions `${sharedRoot}/aws` with mode
0700 under the existing NFS service-identity mapping. An ordered, required
`vibe-kanban-aws-state.service` links each coordinator/worker service home's
`.aws` to that directory before agent execution begins. Coordinator migration
imports existing state without overwriting conflicting files. Local originals
remain in `.aws.before-vk-shared`; workers preserve their local state there but
do not import it into the coordinator's authority. An unexpected symlink or
conflict fails setup rather than replacing credentials.

The entire vendor-managed directory is shared: AWS_CONFIG_FILE alone does not
move the HOME-based SSO cache. The directory remains writable so vendor token
refresh works. This grants the configured cluster service identities access
to the configured profiles; it is not a general promise for arbitrary remote
hosts. Credentials are not sent through workspace dispatch or chat.

`prepare_scoped_home` in `crates/worker/src/execution.rs` symlinks non-config
siblings. Creating `.aws` before an execution home is prepared means a login
or atomic cache-file replacement later remains visible through that overlay.
An already-running turn created before deployment may lack the link: use a
new turn after rollout. Codex only scopes CODEX_HOME and inherits service HOME.

AWS CLI must also be in both coordinator and worker systemd service PATHs.
A host's `/run/current-system/sw/bin/aws` is not proof of worker availability;
including `pkgs.awscli2` in the service paths installs its Nix closure there.

Select a named profile explicitly with `AWS_PROFILE` (or CLI `--profile`).
Signing into an SSO session does not choose a default among its profiles.
Acceptance requires both CLI STS identity and a Node SDK default provider in
an agent shell after rollout; filesystem tests alone cannot prove live AWS
authentication. No token, exported access key, or cache content belongs in logs.

## Contributed by

- vk/c817-aws-sso-sign-in
