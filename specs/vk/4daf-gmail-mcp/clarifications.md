# Clarifications: Gmail MCP connector (`vk/4daf-gmail-mcp`)

Resolutions folded into [`spec.md`](spec.md). No `[NEEDS CLARIFICATION]` markers
remain.

## Resolved during specification

### C1. Does "my fork" mean the fork must be the install source?

**Question.** The requester asked to add *their fork*. That fork is currently
byte-identical to upstream (`ahead_by: 0`, `behind_by: 0`), has one branch, and
publishes no releases. Upstream *is* published to npm as
`@artymclabin/gmail-mcp@1.2.3`. Pointing at npm would be faster and
Renovate-trackable.

**Resolution.** The fork is the install source, pinned by commit SHA
`030da3492753222a41645a9f343466d151c63f3c`.

**Why.** The ask was explicit and unambiguous. A fork with zero divergence today
is a fork intended to diverge tomorrow; pointing at upstream now would mean a
second, user-visible reconfiguration later. The mechanism works: the package
declares `"prepare": "npm run build"`, so npm compiles TypeScript on a git
install — verified, 53 s cold, producing an executable `dist/index.js`.

**Recorded fallback.** `@artymclabin/gmail-mcp@<exact-version>` remains the
documented alternative, named in `SPEC.md`'s rejected alternatives.

### C2. Should the three named mailboxes ship as three catalog entries?

**Question.** The request names Gmail MCP (Personal), (Sweetgreen), (Proalign).
Three catalog rows would be the smallest diff and literally what was asked.

**Resolution.** No. One `gmail` entry, instantiated up to three times by the
user, each renamed.

**Why.** `default_mcp.json` ships to every user of an open-source product. Rows
naming one person's employer and client would publish private affiliation, be
noise for everyone else, and establish that the catalog grows per-user rows. It
also leaves the single-instance limitation intact for every other template,
whereas fixing instantiation generically serves Slack, Context7, and everything
added later. Constitution XXII now states this rule directly.

**What the requester actually gets.** Three servers named by them — for example
`gmail_personal`, `gmail_sweetgreen`, `gmail_proalign` — each with its own
credentials path and tool prefix. Note the identifier rules (`^[a-zA-Z0-9_-]+$`)
mean "Gmail MCP (Personal)" cannot be the stored name; there is no separate
display-label field, and adding one is out of scope because agent-native
configuration has nowhere to persist it.

### C3. Is a separate integrity digest and audit job required, matching Slack?

**Question.** The Slack entry carries `SLACK_MCP_LAUNCHER_SHA256` and a daily
`pinned-artifacts.yml` audit. Should Gmail have the equivalent?

**Resolution.** No digest constant, no audit workflow.

**Why.** The two pins are different in kind. Slack pins a **GitHub release
asset**, which GitHub permits replacing under an existing tag — the pin names a
mutable location, so a recorded digest plus a scheduled re-check is the only
available control, and it is explicitly a detection control. Gmail pins a **git
commit SHA**, which names an immutable object; npm resolves and verifies it at
install time on every user's machine. The SHA *is* the integrity record.

Adding an audit here would re-check a value that cannot change, and an unaudited
constant would be worse than none ("a digest that nothing re-checks is a comment,
not a control"). Constitution XVI now states that a content-addressed pin needs
no companion audit, so a future reviewer does not demand parity with the weaker
mechanism.

## Resolved during clarification

### C4. Should the cold-start build cost be reduced before shipping?

**Question.** A git install builds from source: ~53 s on a cold npm cache,
requiring `git` and the ability to install dev dependencies. A published tarball
or npm package would avoid it. Should this task block on the fork publishing one?

**Resolution.** No. Ship the git SHA pin. Accept the cold-start cost, document it
as a prerequisite, and record the conditions that reopen the decision.

**Why.** The cost is one-off per machine per revision — subsequent launches hit
the npx cache — and the alternative requires work in a repository outside this
task's scope. Correctness does not depend on it.

**Reopen conditions** (stated explicitly, per Constitution XVI's requirement that
an accepted exception name what would change it):

1. The fork publishes a release artifact or a fork-controlled npm package. Then
   move the pin to the exact version, confirm `dist.integrity`, switch Renovate
   to the npm source, and update the entry, tests, and both documentation layers
   in one reviewed change.
2. Users report install failures traceable to `ignore-scripts=true`, a missing
   `git`, or an offline host — the git-install path's three real prerequisites.
3. First-launch latency becomes a support burden in practice.

### C5. Should a Gmail icon ship with the entry?

**Question.** Catalog tiles render an icon from `packages/public/mcp/` when
`meta.<key>.icon` is set, and fall back to the server's first initial otherwise.

**Resolution.** No icon. The entry ships without an `icon` key.

**Why.** The Slack entry already ships without one, so the fallback is an
established look rather than a visible gap. The Gmail logo is a Google trademark;
whether it may be redistributed in this repository is a legal question, not an
engineering one, and the gain is cosmetic. Adding one later is a one-line change
plus an asset, with no migration.

## Remaining open questions

None blocking implementation.

One item is deliberately left to the requester rather than decided here, because
it is a preference rather than a correctness question: **whether the ~53 s first
launch is acceptable, or whether they would rather the entry point at
`@artymclabin/gmail-mcp` for speed at the cost of not being their fork.** C4
resolves it as "ship the fork"; reversing it is a one-line change to the entry
plus its test constant and the docs. This is surfaced in the task summary rather
than left implicit in a spec file.
