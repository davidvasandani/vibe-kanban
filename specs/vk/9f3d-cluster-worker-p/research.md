# Research: Legible Worker Executor Capabilities

Findings that constrain the plan, each verified against the code rather than
assumed.

## R-1 — `BaseCodingAgent::VARIANTS` does not exist

`crates/executors/src/executors/mod.rs:96-107`. `VariantNames` is derived on the
**outer** `CodingAgent` enum. The `strum_discriminants(...)` derive list for
`BaseCodingAgent` is `EnumString, Hash, strum_macros::Display, Serialize,
Deserialize, TS, Type` — no `VariantNames`.

Consequence: the "valid executor names" list for the FR-1 error message must come
from `CodingAgent::VARIANTS`. Both enums carry
`strum(serialize_all = "SCREAMING_SNAKE_CASE")`, so the strings match what
`BaseCodingAgent::from_str` accepts.

**Correction from `/speckit.analyze` (B1)**: `VARIANTS` is an associated const on
the `strum::VariantNames` **trait**, which must be in scope to read.
`crates/worker/Cargo.toml` has no `strum` dependency, and `executors` re-exports
only `strum_macros` (the derive crate, not the trait crate). `grep -rn
"VARIANTS" crates/ --include=*.rs` returns zero hits — nothing in the repo reads
it today. The worker therefore **cannot** read `CodingAgent::VARIANTS` directly.

Resolution: expose `pub fn valid_executor_names() -> String` from the new L1
helper in `crates/executors/src/profile.rs`. The worker already depends on that
crate, L1 already owns the canonical-enumeration concern, and no new top-level
dependency is added (constitution *Constraints*).

Rejected alternative: adding `VariantNames` to the discriminant derive list.
It would work, but `BaseCodingAgent` is a generated-type boundary (`TS`, `Type`,
`sqlx`) and widening its derives to serve an error message is a larger blast
radius than a helper function.

Rejected alternative: adding `strum` to `crates/worker`. A new top-level
dependency for one error message, when a crate already in the graph can answer
it.

Caveat to handle: `QaMock` is `#[cfg(feature = "qa-mode")]`, so `VARIANTS` is
feature-dependent. The message is advisory text, so this is acceptable, but the
test asserting message content must not pin the full list.

## R-2 — `CURSOR_AGENT` has a two-name alias

`crates/executors/src/executors/mod.rs:116-118`:
`#[strum_discriminants(strum(serialize = "CURSOR", serialize = "CURSOR_AGENT"))]`.

With strum, multiple `serialize` attributes make **both** parse via `from_str`.
So `CURSOR` and `CURSOR_AGENT` both resolve, and canonicalisation maps `CURSOR`
→ `CURSOR_AGENT`.

**Correction from `/speckit.analyze` (m11)**: for the preferred output name
strum's `get_preferred_name` picks the **longest** `serialize` value, not the
last. The outcome coincides here (`CURSOR_AGENT` is longer than `CURSOR`), so
canonicalisation is correct — but a future variant whose intended canonical name
is the *shorter* alias would silently canonicalise the wrong way.

Note also that `CodingAgent::VARIANTS` derives from the **outer** enum's
`serialize_all`, not from the discriminant's `serialize` attributes at all. The
two lists agreeing is a coincidence of this codebase, not a guarantee.

Both facts must be pinned by tests rather than trusted, because they are strum
implementation details this feature depends on.

## R-3 — Two independent normalisation conventions already exist

- `de_base_coding_agent_kebab` (`crates/executors/src/profile.rs:74-83`) does
  `raw.replace('-', "_").to_ascii_uppercase()` before `from_str`. It is a serde
  deserializer, so it is unreachable from a plain `&str`.
- `ExecutorProfileId::cache_key()` and its `Display` impl both format
  `EXECUTOR:VARIANT` — duplicated logic (`profile.rs:103-118`).

The new helper must reuse the *first* convention exactly, so an operator writing
`claude-code` in Nix gets the same result as a JSON payload carrying
`"claude-code"`. Deduplicating `cache_key`/`Display` is tempting but out of
scope — it is unrelated to this feature and would widen the diff.

## R-4 — `NoEligibleWorkers` has exactly one non-test reference

`grep` across the workspace: `crates/services/.../scheduler.rs:19` (definition),
`:75` (construction), `:363` (test). The call site
`crates/server/src/routes/workspaces/create.rs:383` consumes it as
`ApiError::BadRequest(error.to_string())` and never matches a variant by name.

Consequence: the variant can be replaced by two variants without touching the
server crate. `SchedulingError` derives `Debug, Error, PartialEq, Eq` only — it
is **not** a `TS` type, so no generated TypeScript changes.

## R-5 — Per-item `disabled` is already supported and styled

`packages/ui/src/components/CreateChatBox.tsx:167-175` renders each executor as
a `DropdownMenuItem`.

**Correction from `/speckit.analyze` (m12)**: that `DropdownMenuItem` is imported
from `./Dropdown` (`CreateChatBox.tsx:6`), i.e.
`packages/ui/src/components/Dropdown.tsx:172-235` — **not** `DropdownMenu.tsx`.
Both exist, both spread Radix `Item` props, and both carry `data-[disabled]`
styling, so the conclusion survives — but the two have different prop surfaces,
and `Dropdown.tsx`'s is the one with the `icon`/`badge`/`variant` props
CreateChatBox actually uses. Editing `DropdownMenu.tsx` would change nothing.

**Correction from `/speckit.analyze` (M6)**: a native `title` tooltip cannot be
the reason carrier. `Dropdown.tsx:205` sets
`data-[disabled]:pointer-events-none`, which suppresses the hover a `title`
requires — the user would see a 50%-opacity row and no explanation. Use the
`badge?: React.ReactNode` prop already on `DropdownMenuItemProps`
(`Dropdown.tsx:175`) so the reason is visible text.

Consequence: FR-7 needs no new styling, only a way for the container to say
which options are unsupported. `ExecutorProps` is
`{ selected, options: TExecutor[], onChange }` — a bare string array — so the
prop must be widened.

Chosen shape: add optional `unsupported?: ReadonlyMap<TExecutor, string>` (value
is the reason) to `ExecutorProps`. Optional keeps every existing caller
compiling and keeps remote-web unaffected until it opts in.

**Addendum from `/speckit.analyze` (M9)**: `ExecutorProps` has a **second**
consumer — `packages/ui/src/components/SessionChatBox.tsx:20` imports the type
and renders its own executor dropdown at `:793-797`. Widening the interface
changes that component's public contract without changing its behaviour (it
simply ignores the new optional field). That is acceptable and is why the field
must be optional, but it is a constitution-IV blast-radius fact the original
research missed.

Rejected alternative: changing `options` to an object array. It is a breaking
change to a `@vibe/ui` primitive consumed by both frontends (constitution IV
makes both the blast radius) for no gain over an optional sibling field.

Rejected alternative: encoding unavailability through `formatExecutorLabel` by
appending "(unavailable)". It cannot disable the item, and it puts state into a
presentation callback.

## R-6 — `capabilities` reaches the browser as `unknown`

`crates/db/src/models/worker_node.rs:46` — `#[ts(type = "unknown")]` on
`capabilities`, `resource_snapshot`, and `labels`.

Consequence: FR-8's defensive parsing is not optional politeness; the type
system provides nothing here. The existing frontend test fixture
(`WorkersSettingsSection.test.tsx:78`) uses
`capabilities: { executor_profiles: ['codex'] }` — note the **lowercase** value,
confirming that non-canonical data is what the frontend has historically seen.
The client-side helper must therefore canonicalise too, not just the backend.

## R-7 — Existing worker config test omits the profiles variable

`crates/worker/src/lib.rs:219-243`,
`parses_required_identity_and_coordinator_with_safe_defaults`, sets neither
`VK_WORKER_EXECUTOR_PROFILES` nor asserts on it. FR-2 makes it fail.

This is the correct outcome and the test must be updated, not worked around: it
currently encodes "a worker with no executors is a valid default", which is the
defect. The update is itself evidence for the acceptance criterion.

## R-8 — Scheduler test fixtures use data the system cannot produce

`crates/services/.../scheduler.rs:177` advertises `["codex", "claude"]` and every
test requests bare lowercase `"codex"`. Neither is producible: requests always
arrive as `Display` output (`CODEX:DEFAULT`), and `"claude"` is not an executor
name at all (`CLAUDE_CODE` is).

The fixtures pass today only because both sides of the comparison are equally
unrealistic. They must be moved to canonical values *before* the legacy-tolerance
test is added, otherwise that test proves nothing — every existing test would
already be exercising the legacy path.

`/speckit.analyze` (m18) found the same class at
`crates/worker/src/server.rs:410` (`executor_profiles: vec!["codex".into()]`),
which is now also in scope. The empty-capability fixtures in `reconcile.rs:334`
and `registry.rs:329` are left alone — they exercise registry plumbing, not
matching, and an empty list is still representable on the wire.

## R-9 — Capabilities are written only at registration, never at heartbeat

Verified during `/speckit.analyze` (m13). `WorkerHeartbeat`
(`crates/cluster-protocol/src/lib.rs:80-85`) has no capabilities field, and the
registry's heartbeat path preserves the stored value
(`crates/services/.../registry.rs:118`, `capabilities: current.capabilities.0`).
Only `register` (`crates/worker/src/server.rs:139-143`) writes them.

Consequence: FR-4's tolerance for non-canonical stored rows is **permanent**, not
a transient upgrade window. A coordinator-only restart — the normal homelab
deploy shape — leaves worker rows untouched for those workers' entire uptime.
This is the single most load-bearing fact behind FR-4 and it was missed in the
first pass.
