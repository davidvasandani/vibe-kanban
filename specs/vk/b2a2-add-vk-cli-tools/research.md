# Research: CLI Tools in Workspace Sessions

## Decision 1: derive managed paths on the process-owning host

**Decision**: local spawners and cluster workers derive their own managed CLI
bin path immediately before child-process creation.

**Why**: the worker owns process creation and has node-local service state. The
coordinator's absolute application-data path is not guaranteed to identify the
same object on a worker. This follows the clustered execution and cross-node
path constitution.

**Rejected**: append the coordinator path to dispatch/request environment.
That advertises an unproven path on another node and can shadow a valid worker
configuration with unusable data.

## Decision 2: no install distribution in this feature

**Decision**: augment PATH only when the execution host's managed bin directory
exists.

**Why**: CLI Tools is machine-scoped. Synchronizing installs introduces artifact
distribution, version convergence, partial-failure, removal, and credential
boundary questions absent from the request.

**Rejected**: mount or copy the coordinator's CLI tools tree to workers. This is
a separate deployment feature with materially larger lifecycle and security
scope.

## Decision 3: share path assembly below `services`

**Decision**: place the reusable augmentation helper in `utils`, alongside
`assets::cli_tools_dir` and `shell::merge_paths`.

**Why**: both `local-deployment` and `worker` already depend on `utils`; the
worker does not depend on the catalog/install service and should not gain that
large dependency merely to identify a public bin directory.

**Rejected**: duplicate `cli_tools_dir().join("bin")` and existence/merge logic
in each spawner. That violates the one-convention-per-concept rule and makes
ordering drift likely.

## Decision 4: preserve host-first ordering

**Decision**: treat the current/inherited PATH as primary and the managed bin as
secondary using the existing de-duplicating merge helper.

**Why**: this is the behavior promised by the CLI Tools settings UI and recorded
in project knowledge. It also makes dual host/app provisioning safe.

**Rejected**: prepend managed tools. That would silently replace machine-policy
versions and contradict existing product copy.

## Decision 5: no new dependency or wire contract

The current crates already expose all required primitives. A protocol field for
the managed path would encode the wrong ownership boundary, while a boolean is
unnecessary because directory presence naturally makes augmentation a no-op.
