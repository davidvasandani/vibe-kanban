<!--
SpecKit project constitution (vibe-kanban).
The Specify / Plan / Analyze stages read this file and check work against it.
-->

# Project Constitution — vibe-kanban

## Core Principles

### I. Clarity over cleverness
Code and specs are written to be read. Prefer the obvious solution; match the
comment density, naming, and idiom of the surrounding code. Justify any
non-obvious choice in the spec or plan.

### II. Test the contract
Every feature defines how we will know it works (acceptance criteria) before it
is implemented. Rust logic gets `#[cfg(test)]` unit tests; UI/section changes
get a rendered-DOM component test where one already exists for that surface. No
feature is "done" without a checkable validation.

### III. Small, reversible steps
Ship the smallest change that delivers value. Prefer reusing an existing
component and an existing data source over adding new plumbing. When behaviour
already exists for one case, generalise it rather than duplicating it. Avoid
speculative generality.

### IV. Shared-component boundaries are law
The frontend has two shared tiers: `packages/ui` (`@vibe/ui`) owns primitive
presentational components (Button, Dialog, Badge, etc.) and their own internal
layout; `packages/web-core` owns shared feature containers and data-fetching
logic used by both local-web and remote-web. Containers in `web-core` supply
data to `packages/ui` primitives; they do not reimplement presentation.
A change to either shared package affects both `local-web` and `remote-web` —
treat both frontends as the blast radius. Styling guidance lives in
`packages/local-web/AGENTS.md`.

### V. Remote mutations are transactional and txid-covered
On the `crates/remote` server the read path is ElectricSQL shapes; the write
path is REST handlers. Every mutation runs its DB work inside one Postgres
transaction and returns the `txid` the client waits on before dropping optimistic
state. Any side effect triggered by a mutation (e.g. archiving workspaces when an
issue's status changes) MUST run on the same transaction connection as the
triggering write, so it commits atomically and is covered by that one `txid`.
Per-project `project_statuses` are user-customisable and carry no terminal flag;
existing code identifies terminal statuses ("Done", "Cancelled") by
case-insensitive name — follow that established convention rather than inventing a
new category concept.

### VI. Don't rebuild what shipped
Extend existing machinery instead of forking it. Before adding a code path,
search for one that already does the job (git history, the knowledge base, the
crate's `AGENTS.md`) and build on it.

### VII. Workspace breadcrumbs preserve issue identity
Any workspace breadcrumb that represents or links through an issue MUST display
the issue ID. The ID is the stable cross-view reference for task identity, so it
must remain visible even when the workspace title, issue title, or responsive
layout is shortened.

### VIII. Managed tools are pinned, verified, and user-owned
Managed CLI catalog entries are a supply-chain boundary. Each tool must have a
stable wire identifier, deterministic install location, pinned version, official
source link, platform-specific artifact mapping, exact SHA-256 verification, and
clear unsupported-host behavior. Installs and updates must use the existing
staged atomic workflow so failed downloads, checksum mismatches, or extraction
errors never leave partial executables on an agent's PATH. Tool credentials and
configuration remain user/host managed unless a spec explicitly and safely
defines otherwise.

### IX. External agent protocols are defensive contracts
Coding-agent integrations use the vendor's documented noninteractive protocol,
preserve stable serialized executor identifiers, and parse structured output
without assuming the event schema is closed. Unknown events must degrade safely;
session identity, cancellation, failures, and credential redaction must remain
correct. Extend the shared executor, log-normalization, profile, and MCP
abstractions before introducing agent-specific parallel machinery.

Normalized-log compaction must preserve protocol lifecycle identity and patch
ordering. Repeated events may share a visible entry only when semantic equality,
adjacency, and completion state are proven; failures stay visible, stale event
updates cannot overwrite newer occurrences, and compact indicators remain
bounded under arbitrarily long streams.

### X. Dialogs hold provisional state; containers hold confirmed state
Settings dialogs and edit modals own a private snapshot of the data they mutate.
On open, the dialog is seeded from the current saved state (or blank for "add").
The dialog may freely mutate its own local copy. Only an explicit submit action
writes the complete, validated result back to the persistent draft or store.
Close and cancel must discard all modal-local changes without touching the outer
state. Inline mutations of shared draft state from inside an open form are
disallowed. This applies to MCP server definitions, agent assignments, and any
other compound form that edits a named object inside a larger collection.

### XI. Diagnostics are evidence, not decoration
Tool, executor, integration, and MCP diagnostics are user-facing evidence for
debugging. UIs that surface diagnostics MUST preserve the exact backend-provided
text for display, copy, and issue/task seeding, including line breaks and long
unbroken content, while treating that text as inert and untrusted. Diagnostic
actions may add surrounding context, but they must not truncate, reinterpret,
auto-remediate, or silently discard the original diagnostic.

### XII. Asynchronous handoffs have one authoritative owner
State handed from a request to an asynchronous lifecycle consumer (queued work,
follow-up execution, cleanup, or similar) MUST have an explicit claim boundary.
The producer and consumer must coordinate against authoritative backend state so
a stale client observation cannot strand work, and concurrent paths cannot both
perform it. Avoid holding coordination locks across awaited external or process
operations; claim under synchronization, then perform the work after releasing
it. Regression tests cover both orderings at the handoff boundary.

### XIII. Vendor config files are edited, never owned
When a feature manages entries in a config file that belongs to an external
tool in the user's home directory (e.g. AWS SSO profiles in `~/.aws/config`),
Vibe Kanban is a guest editor, not the file's owner. Writes touch only the
managed sections and preserve everything else byte-for-byte (unknown sections,
keys, comments, ordering); writes are atomic (temp file + rename) with
restrictive permissions on create; a file the editor cannot parse is never
rewritten. Only non-secret configuration is written — credentials and tokens
are acquired by launching the vendor CLI's own login flow in a signed,
machine-scoped PTY (the established managed-CLI login boundary) and live only
in the vendor's storage. Every field is validated server-side before any
write; the browser never supplies command strings or raw file content, and
command exit is never equated with verified authentication.

### XIV. Repository verification is worktree-safe
Repository-mandated verification commands must behave predictably in a fresh
development worktree. Required tool dependencies are either bootstrapped through
the repository's locked dependency graph or checked before any multi-stage
verification begins, with an actionable setup command on failure. Verification
must never silently skip a language or package after reporting overall success.

### XV. Destructive operations fail safe and are loud
Any code path that can delete, reset, or overwrite a user's working tree treats
uncommitted work as irreplaceable. Such a path MUST: establish that the target
holds no unsaved work before acting; **retain** rather than destroy when that
cannot be determined (an error is never evidence of emptiness); and log the
target, the reason it was selected, and the action taken at `info!` or above
*before* acting — a destructive step logged only at `debug!` is invisible in
production and does not satisfy this. Where destruction is genuinely required,
move the data aside (e.g. a `.recovered-<epoch>` sibling) or snapshot it to a
commit rather than deleting it outright.

Work-preservation must not be conditional on an unrelated step succeeding.
Preservation and teardown are independent concerns: a failure to stop a process,
to reach a repository, or to clean up metadata must never skip the preservation
of that unit's uncommitted work. Fail-safe direction must be consistent across
sibling cleanup paths; two routines that decide the same question must not
disagree about which way to err.

### XVI. Bundled third-party entries install what they advertise
Anything the product suggests launching on a user's machine — preconfigured MCP
catalog entries included — names an **immutable, version-addressed artifact**
(a released version, tag, or digest; never `@latest`, a branch, or a mutable
tag), and that artifact must come from the same repository the catalog metadata
links to. Source URL and executable source are one claim, not two.

Integrity must be enforced before any code from an artifact outside the trusted
delivery mechanism executes. A digest that is checked only later by scheduled
CI is a detection control, not install-time verification. If prevention cannot
be shipped because it depends on unavailable external ownership or credentials,
the exception must be explicit: document the residual threat, controls,
notification path, and concrete condition that reopens the decision. Dependency
updates move the source pin, integrity record, tests, and documentation in the
same reviewed change and fail closed rather than substituting another build.

When a bundled entry points to an operator-hosted shared service instead of
launching locally, the deployment remains responsible for that same immutable
source and integrity contract. The catalog contains only the network endpoint
and non-secret headers; upstream credentials stay at the supervised service and
must never be copied into an agent's native configuration.

### XVII. Live capability state is confirmed and atomic
Configuration on disk is not evidence that a running external agent adopted a
change. Any feature that reports a live tool, connector, or protocol capability
as refreshed MUST receive confirmation from the process that owns that live
capability set. Unsupported in-process reload paths are reported truthfully
rather than simulated with an independent probe. A product operation may instead
offer an explicitly named agent restart: it must preserve the logical session,
use the normal continuation path, and report success only after a fresh process
has started from the new configuration. It must never label a restart as a live
refresh.

Capability replacement is generation-based: readers observe one complete old or
new inventory, never a partially rebuilt set. Refresh coordinates with in-flight
calls, preserves last known-good capability state on partial failure, and
identifies failures by stable configured identifier. Configuration comparisons,
logs, diagnostics, and API results never expose environment values, tokens,
authorization material, authenticated URLs, or secret-bearing command arguments.

If a restart is requested while a turn is running, the product must obtain an
explicit user confirmation before queueing the handoff. The current turn is not
interrupted unless the dialog says so. Exactly one lifecycle consumer owns the
queued continuation.

### XVIII. Distributed execution is affinity-bound and evidence-backed
Workspace process ownership MUST be explicit, persisted, and stable. A
coordinator may dispatch work only to the worker assigned to that workspace,
and the worker must authorize the execution ID and canonical workspace path
against that assignment. Retries are idempotent and cannot create a second
process for one execution.

Remote liveness and terminal state require worker evidence. A timeout,
disconnect, missing handle, or expired lease is not proof that a process
completed or was killed; expose interruption or indeterminacy and preserve the
workspace until reconciliation establishes safety. Ordered event streams carry
monotonic cursors and make replay gaps visible. Shared Git worktree
administration remains single-owner and serialized even when ordinary commands
run on several nodes.

### XIX. Observability is a read-only surface
Metrics, telemetry, and diagnostic sampling exist to be *looked at*. They are
never evidence.

No observability path may write scheduling, liveness, lease, eligibility, or
lifecycle state, and no lifecycle decision may read from one. A node that fails
to report metrics is not offline; a node that reports them is not healthy. The
existing evidence channel remains the only authority on both questions.

Absence is typed, never fabricated. Unreachable, unsupported, not-implemented,
and stale are distinct statuses carrying their reason, and each renders as
itself. A zero that means "no reading" is prohibited — a failed read is not a
measurement, and a UI that shows `0%` for a dead host is a defect.

Live streams are bounded and self-correcting. Retention is a fixed-size window
whose memory does not grow with uptime, and no emitted payload may grow with
elapsed time. A patch stream is an optimisation over a periodic full snapshot,
never a replacement for one: a dropped message, a replay gap, or a change in the
member set forces a resnapshot rather than interpolation. Every streamed
collection is keyed by stable identity — never by array position — so that
membership changing mid-stream cannot make a `replace` land on the wrong row.

Sampling tasks terminate. A background sampler holds only a weak reference to
its owner, re-checks each tick that a consumer still exists, exits when none
does, and never holds a lock across an await.

Host introspection is secret-hostile by default. Process environments are never
read. Anything derived from a process command line is redacted at the point of
collection — before it is stored, transmitted, or logged — so that an
unredacted value never exists outside the sampler. Redaction errs toward
removing too much: an over-redacted command is cosmetic, an under-redacted one
is a disclosure.

### XX. Cross-node paths are node-identical and structurally verified
Any absolute path written into shared storage that another node must later
resolve MUST resolve to the same object on every node, and that property MUST be
asserted by the code that records it — never left to an operator convention, a
documentation note, or a naming coincidence.

Three rules follow. **Verify structure, not spelling:** assert that a resolved
target lies within the shared root, never that its text lacks a known-bad prefix.
**A same-named local directory is not the target:** existence proves nothing, and
a resolver that accepts a local path merely because it exists is a defect, not a
fallback — the shared-mount rule applied to every recorded path. **Both ends of a
two-sided pointer are repaired and re-probed together;** a zero exit from a repair
command is not verification, and an object a path claims to reference is proven
present, never assumed.

Enforcement is level-triggered. A check that runs only where the path is first
written is an edge trigger and will stall silently; the same assertion runs at
startup, at placement, and before use, enumerating every violation in one pass
with an actionable remedy rather than aborting on the first. A one-off migration
with no recurring check is a comment, not a control.

Where a shared namespace is consolidated, its blast radius is re-derived rather
than inherited: an operation that was safe while it touched one node's metadata
is not automatically safe once every node's metadata lives in one place.
Writes into such a namespace are **additive by default**: an operation that
deletes or prunes entries there needs an argument for why every other holder of
the namespace, on every node, is unaffected.

### XXI. One convention per concept, and failures say what failed
A value that already has a resolution rule in this codebase is resolved by that
rule everywhere it is consumed. Re-deriving the rule at a new call site — a
second string format, a narrower lookup, an extra normalisation — is a defect
even when it passes its own tests, because the two definitions will disagree on
exactly the inputs the original rule exists to handle. Find the existing
resolver, call it, or match its outcome exactly and say so in a comment naming
it.

Consumers must accept the full domain the producer emits. Where a producer is
user-facing (a picker, an API request body, a config field), the domain includes
its *default* value, and the default is the case most likely to reach
production — a consumer that handles every case except the default is broken for
almost every user.

A failure that a maintainer could act on must reach the operator with the fact
that identifies it. Collapsing a specific, diagnosable failure into a generic
message ("an internal error occurred") is a defect in its own right: it converts
a one-line diagnosis into an investigation, and it does so precisely when the
system is already failing. Server errors keep their status but carry a message
naming what failed and which entity it failed for. Widening an error channel is
scoped to the failure being surfaced — a blanket unwrapping of every internal
error is not the remedy, and messages remain free of secrets, tokens, and
environment values.

### XXII. Affinity migration is a single owned lifecycle transition
An executing process is never transferred between nodes. Changing affinity
while work is active means terminating the old execution with the established
stop/cancellation protocol, committing the new placement, and creating at most
one new continuation execution. The coordinator owns that full transition; a
browser or worker may request it but must not assemble it from independent
stop, placement, and follow-up mutations.

The transition is serialized per workspace and revalidates both liveness and
target eligibility at execution time. A missing worker response is not evidence
that stop succeeded, and affinity cannot change until terminal evidence meets
the existing lifecycle contract. Duplicate requests, retries, and lost HTTP
responses cannot create duplicate continuations.

Outcomes name the last durable boundary reached. If stop fails, placement is
unchanged. If placement succeeds but continuation creation fails, the workspace
remains stopped on the new affinity and the operator is told exactly that; the
system must not roll back into a node that may still own process state or claim
the migration completed. Product-owned continuation prompts are versioned in
source and preserve the user's prior session context rather than fabricating a
new task.

### XXIII. Remote execution receives authoritative configuration snapshots
Configuration that affects a remotely owned execution is resolved by the
coordinator and carried through the authenticated dispatch boundary. A worker
must not reconstruct settings from deployment defaults, query an unauthenticated
side channel, or silently continue with stale local state when the coordinator
supplied a snapshot.

Secret-bearing snapshots are minimal, bounded, included in idempotency checks,
and never logged or returned in diagnostics. Workers validate that a snapshot
belongs to the dispatched executor, apply it atomically through the existing
native-config adapter, and preserve unrelated vendor settings. Optional protocol
fields provide rolling compatibility; presence with invalid content fails closed
before the child process starts.

### XXIV. MCP definitions have one settings authority
Vibe Kanban settings own every MCP definition. Deployment configuration may
install immutable executables, expose network routes, and supply service
environment, but it must not add, remove, or rewrite native agent MCP tables at
service startup. Remote workers consume settings-owned definitions through the
authenticated execution snapshot rather than reconstructing them locally.

### XXV. Executor continuation artifacts move by manifest, never by home copy
An executor home mixes immutable continuation artifacts with mutable databases,
credentials, configuration, caches, and logs. Cross-node continuation transfer
MUST select only artifacts required for the named continuation, bind them to an
operation manifest, and verify identity, size, digest, regular-file type,
permissions, ownership, and structural containment before stopping the source
or changing affinity. Copying the complete executor home is prohibited.

Transfer completion is durable positive evidence, never path existence. Retries
reuse identical verified content and reject conflicts rather than overwriting.
Missing, corrupt, oversized, unauthorized, indeterminate, or partial lineage
leaves source execution and placement unchanged. Readers, writers, and cleaners
never follow symlinks, accept caller absolute paths, or log contents; all byte,
count, depth, and time dimensions are bounded. Partials are operation-scoped,
and verified artifacts have age-based retention that protects every active or
recoverable reference.

### XXVI. Collapsed controls retain decisive context
Expandable workspace controls MAY hide detail, but their collapsed affordance
MUST retain the concise state needed to decide whether to open them. That state
comes from an existing summary/cache source when one exists; a closed section
must not stay mounted or issue a private request solely to label its header.
Header metadata and disclosure actions share a constrained row, so dynamic text
must have an explicit shrink/truncation boundary and remain distinguishable from
the control label at the narrowest supported sidebar width.

### XXVII. Legacy identifiers migrate explicitly and atomically
When a persisted external identifier no longer satisfies the protocol contract,
reads may diagnose and propose the shared canonical form but must not mutate
storage. Migration occurs at an explicit write boundary, preserves the original
human label and complete definition, rejects collisions and cross-profile
ambiguity before the first write, and either commits every affected native key
plus metadata or leaves the legacy state recoverable and clearly reported.

### XXVIII. Accepted lifecycle work outlives its request
Once an API acknowledges lifecycle work that creates or mutates durable product
state, completion MUST NOT depend on the originating HTTP connection, browser
route, or component remaining alive. The coordinator persists an authoritative
operation identity and observable state before acknowledgement, and exactly one
background consumer claims the work. Retries replay or continue that identity;
they never create a duplicate execution or repeat a committed phase.

Pending, successful, failed, and indeterminate outcomes are durable and visible
through ordinary product reads. Process restart reconciles every accepted
non-terminal operation from positive evidence: it resumes an idempotent phase or
records a truthful actionable outcome, never infers success from absence and
never leaves an unbounded pending state. Slow external or filesystem operations
run outside request cancellation, and no coordination lock is held across them.

## Constraints
- Follow the existing architecture and conventions of the repository.
- Do not introduce new top-level dependencies without recording the reason in
  the plan's research notes.
- Generated files (`shared/types.ts`, `shared/remote-types.ts`) are never edited
  by hand; regenerate via the `generate-types` scripts.
- Managed CLI catalog additions must preserve existing wire identifiers,
  host-copy precedence, removal/update behavior, and spawned-agent PATH
  semantics.
- Executor additions must include generated-contract regeneration, exhaustive
  backend/frontend mapping checks, and fixture-based protocol tests.
- Diagnostic issue/task creation must use an explicit current project or
  workspace context; never choose an arbitrary project when context is missing.
- Preconfigured MCP catalog entries stay transport-neutral (`command`, `args`,
  `env` with credential placeholders); per-agent shape is the adapter's job.
- Run `pnpm run format` before completing a task.

## Governance
This constitution supersedes ad-hoc preferences. When a spec or plan conflicts
with it, the constitution wins or the conflict is recorded as an open question.

**Version**: 0.25.0 (requires accepted lifecycle work to have a persisted,
single-consumer operation identity, request-independent execution, idempotent
restart reconciliation, and durable user-visible outcomes; 0.24.0 required explicit, collision-safe, atomic legacy identifier
migration with label and definition preservation; 0.23.0 required collapsed controls to retain summary-backed,
responsive decision context; 0.22.0 made Vibe Kanban settings the sole
MCP-definition authority and limited deployment ownership to runtime
prerequisites; 0.21.0 added coordinator-authoritative, bounded remote execution
configuration snapshots with atomic worker materialization and secret-safe
failure behavior; 0.20.0 added coordinator-owned, serialized affinity migration with
truthful durable-boundary outcomes and at-most-once continuation; 0.19.0 added
one-convention-per-concept — reuse the existing
resolution rule rather than re-deriving it, accept the producer's default value,
and report failures with the fact that identifies them instead of a generic
internal error; also makes writes into a consolidated shared namespace additive
by default; 0.18.0 added cross-node path portability — node-identical shared
paths, structural rather than textual assertions, no same-named-local fallback,
two-sided pointer repair, level-triggered enforcement, and re-derived blast
radius for consolidated namespaces; 0.17.0 added observability as a read-only
surface; 0.16.0 added affinity-bound, evidence-backed distributed execution)
