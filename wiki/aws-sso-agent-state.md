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
Login shells rebuild PATH from the system profile, so the rebuild module also
includes AWS CLI in `environment.systemPackages`. Validate login and non-login
shells separately.

Select a named profile explicitly with `AWS_PROFILE` (or CLI `--profile`).
Signing into an SSO session does not choose a default among its profiles.
Acceptance requires both CLI STS identity and a Node SDK default provider in
an agent shell after rollout; filesystem tests alone cannot prove live AWS
authentication. No token, exported access key, or cache content belongs in logs.

## Authentication status probes need a shared concurrency budget

A successful SSO login and a completed STS identity check are distinct events.
The original status endpoint used `join_all` to start an AWS CLI process for
every profile with a five-second deadline. On the coordinator, one probe
succeeded in 1.23 seconds, while 30 of 31 simultaneous probes timed out. Running
four at a time passed all 31 in 15.89 seconds (slowest probe: 3.1 seconds).
The error was process contention, not failed login or inaccessible credentials.

`probe_profile_auth` now shares four permits across the entire server process,
including overlapping list requests and post-login verification. Admission can
wait up to 30 seconds; admitted work then receives its own 15-second execution
budget. A queue timeout reports busy capacity, while an execution timeout says
how long the actual check was allowed to run. `join_all` still preserves the
profile/result ordering; futures waiting for permits do not spawn processes.
The permit and kill-on-drop CLI guard both release on request cancellation.

Do not revert to a per-request limit: two refreshes would multiply the process
count. Do not start the execution timer while a probe is waiting in the queue:
healthy queued profiles would again appear unknown without being checked.

### Production verification limit

Task `vk/c817-aws-sso-sign-in` deployed the bounded probe fix in PR #304
(revision `d154bab`). On 2026-09-17, an isolated live refresh still returned
4 authenticated and 27 unknown profiles while the service's three-CPU cgroup
experienced high CPU pressure and ran 19 Git children. Bounded concurrency
prevents probe fan-out; it does not guarantee completion under competing
service work. Keep successful host-to-agent credential checks distinct from
all-profile status acceptance, and record busy admission separately from
execution timeout. Investigate the competing work before simply increasing
timeouts or declaring the status panel healthy.

### Admission windows for large profile lists

Follow-up from task `vk/c817-aws-sso-sign-in`: eagerly polling all profile
futures starts every queue deadline together. A shared semaphore alone can
therefore reject healthy late profiles behind their own batch. Feed at most
four futures at a time into that semaphore using an ordered buffered stream.
This preserves result association and process-wide limits while allowing a
large refresh to exceed one profile's admission timeout overall. Each refresh
also shares an async cell for executable discovery, avoiding one redundant
`aws --version` subprocess per profile without caching paths across refreshes.
The long-batch regression must exceed the admission deadline and overlap
another refresh; short simulated probes would hide this failure.

### Deployed lazy-admission result

PR #306 (`6cb1d71`) was checked on 2026-09-17 at 17:25 UTC. All 31
profiles returned unauthenticated, with zero unknown/busy/timeout statuses.
A sanitized agent CLI request independently confirmed the shared SSO session
had expired. Refresh SSO before evaluating authenticated acceptance after a
long deployment; completed status checks and successful authentication are
different assertions.

### Final authenticated acceptance

After a fresh Settings sign-in, deployed revision `679b284` (including PR #306)
returned all 31 profiles authenticated with zero unknown statuses. Both the
agent CLI STS call and Node SDK default provider succeeded with ambient static
access keys removed. Task `vk/c817-aws-sso-sign-in` therefore verified the full
Settings → shared AWS state → agent CLI/SDK path, as well as profile reporting.

## Contributed by

- vk/c817-aws-sso-sign-in
