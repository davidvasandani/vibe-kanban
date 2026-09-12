# Research Notes: Gmail MCP connector (`vk/4daf-gmail-mcp`)

## R0. Measured behaviour of the pinned revision

Everything below was executed against
`github:davidvasandani/Gmail-MCP-Server#030da3492753222a41645a9f343466d151c63f3c`
with an isolated `npm_config_cache`, not inferred from documentation.

| Observation | Result |
| --- | --- |
| Cold install | Succeeds in ~53 s, 131 packages, exit 0 |
| Build artifact | `dist/index.js`, 78 KB, executable — confirms `prepare` runs on a git spec |
| `initialize` | `serverInfo: {"name":"gmail","version":"1.0.0"}` |
| `tools/list` | **28** tools |
| `--tool-prefix=personal_` | Every returned tool name carries the prefix |
| Missing `gcp-oauth.keys.json` | Process **exits before `initialize`**, stderr names the missing path |
| Keys file present but invalid, no `credentials.json` | `tools/list` still succeeds |

Two of these contradict the documentation and matter downstream:

- **The README's tool list undercounts.** It enumerates 26; the server registers
  28, adding `get_thread` and `list_inbox_threads`. Documentation and any
  tool-budget reasoning should use 28.
- **Missing OAuth keys is a hard failure, not a degraded mode.** Vibe Kanban's
  "Test connection" will report `failed` for any user who adds the entry before
  creating a Google Cloud OAuth client. That is correct behaviour, but it reads
  as a broken connector unless the docs lead with the prerequisite.

A third consequence follows from the last two rows: because `tools/list` works
with a merely *present* keys file and never touches `credentials.json`, a user
who completes the OAuth-client step but skips per-mailbox consent gets a
connector that **tests green and fails at first use**. The docs give the consent
step equal weight for this reason.

## R1. Which install source — the fork, or upstream npm?

**Facts.** The fork `davidvasandani/Gmail-MCP-Server` is byte-identical to
upstream `ArtyMcLabin/Gmail-MCP-Server` (`ahead_by: 0`, `behind_by: 0`), has one
branch (`main` @ `030da34…`), and publishes **no** GitHub releases. Upstream is
published to npm as `@artymclabin/gmail-mcp`, latest `1.2.3` (10 July 2026).
`package.json` declares `"prepare": "npm run build"` and `"prepublishOnly"`.

**Decision.** Pin the fork by commit SHA.

**Why.** The request was explicit — "add my fork". A fork with zero divergence is
a fork intended to diverge; pointing at upstream now guarantees a second,
user-visible reconfiguration later. The `prepare` script makes the git-spec
install produce a working artifact (verified in R0), so choosing the fork costs
nothing in correctness.

**Trade-off accepted.** Build-from-source on a cold cache (~53 s), requiring
`git` and dev-dependency installation. Mitigated by npx caching after first
launch, documented as a prerequisite, and given explicit reopen conditions in
`clarifications.md` C4.

**Alternative retained as fallback.** `@artymclabin/gmail-mcp@<exact-version>`:
faster, no build step, npm verifies `dist.integrity`, and Renovate could track
it. The packaging knowledge base names an exact registry version as the
*preferred* shape. It loses only on the explicit request.

## R2. Why the Slack delivery idiom does not transfer

`docs/knowledge-base/forked-mcp-server-packaging.md` lists
`npx github:owner/repo#<sha>` in its rejected-alternatives table:

> Pinned, but clones the whole (Go) repo per cache miss and still needs a binary
> source at run time.

Both halves are properties of the **Slack fork specifically**, which is a Go
program. A git checkout of Go source is not runnable by `npx`, so that fork
needed a launcher that downloads a compiled per-platform binary — hence the
tarball, the baked-in digest table, and the whole two-digest apparatus.

Gmail's fork is a TypeScript npm package with a `prepare` script. The checkout
*is* the runnable artifact; there is no second binary source and nothing to
download at run time. The objection's premise is absent.

This is recorded rather than assumed because the divergence is visible: a
reviewer comparing the two catalog entries will see one use a release tarball and
the other a git spec. Constitution XVI now states the general rule — match the
delivery mechanism to the artifact, and record the divergence when a prior
rejection's reason does not apply.

## R3. Why no digest constant and no audit workflow

The Slack entry carries `SLACK_MCP_LAUNCHER_SHA256` plus a daily
`.github/workflows/pinned-artifacts.yml` job that opens a GitHub issue on
mismatch. Gmail gets neither. This is a considered asymmetry, not an omission.

The two pins differ in kind:

| | Slack | Gmail |
| --- | --- | --- |
| Pin names | a release **asset** under a tag | a **git commit** |
| Can its contents change? | Yes — GitHub permits replacing a release asset under an existing tag | No — a commit SHA is content-addressed |
| Integrity enforced when? | Never at install time; npm fetches the URL with no expected integrity | At install time, by npm/git resolving the SHA, on every machine |
| Therefore needs | a recorded digest + scheduled re-check (a *detection* control) | nothing further — the pin is the integrity record |

The knowledge base's own rule decides it: *"A digest that nothing re-checks on a
schedule is a comment, not a control."* Adding a Gmail constant would either be
unaudited (a comment) or audited by a job re-checking a value that cannot change
(ceremony). Constitution XVI was clarified in stage 4 to state this directly, so
a future reviewer does not demand parity with the weaker mechanism.

The residual risk that remains is the same for both and is not addressable here:
if the fork's repository is deleted, the install fails closed — loudly, at agent
launch. It cannot silently resolve to different code.

## R4. Renovate — why no manager is added

Renovate cannot track a bare commit SHA on a fork with no releases: there is no
datasource to compare against. The temptation is to add a custom manager anyway
for symmetry with the Slack entry.

The knowledge base warns against precisely this shape of non-coverage:

> Renovate needs `ignoreUnstable: false` for fork tags like `v1.3.0-vk.1`: they
> are semver **prereleases**, and the default stability filter makes the manager
> match the pin and then never propose anything — **coverage that looks real and
> is not.**

A manager that matches the Gmail SHA and can never propose a successor is the
same failure with a different cause. The pin is therefore documented in
`AGENTS.md` as manually bumped. A known-manual pin is safer than fictitious
automation, and C4's reopen conditions describe when it becomes automatable.

If a Gmail manager is ever added, scope the pre-existing Slack `packageRules`
with `matchFileNames` first — otherwise the older rule's `prBodyNotes` will give
confidently wrong instructions about the new dependency.

## R5. Why multi-instance support is a prerequisite, not a nicety

Google's OAuth model makes one server process strictly one mailbox: the server
loads a single refresh token from a single `GMAIL_CREDENTIALS_PATH`. Several
mailboxes therefore require several processes.

Vibe Kanban cannot express that today. The catalog is keyed by server id and the
id is used verbatim as the logical server name, so a second Gmail cannot be
created — and the backend independently forbids the alternative: same name with
different credentials is reconciled as a **conflict**, not as two servers
(`equivalent_slack_conflicts_on_semantic_stdio_differences` covers the differing
token-value case explicitly). Per-account credentials therefore *require*
per-account names, mechanically.

A distinct `--tool-prefix` per instance is required for a second, independent
reason. Per the upstream README:

> Some MCP clients dedupe tool entries by their base name across servers, which
> makes it impossible to run two instances of this server side-by-side.

This failure is silent: the user sees a working `search_emails` that reads the
wrong mailbox. It is the reason the prefix ships as a required placeholder rather
than being left for users to discover, and the reason Constitution XXII requires
the disambiguator to be part of the shipped configuration.

## R6. No new dependencies

None added, in either language. The change is one JSON entry, one pure
TypeScript function, two frontend call sites, and tests. Recorded explicitly
because the constitution requires new top-level dependencies to be justified
here — there are none to justify.

## R7. Secret handling — a path, not a token

Every other credential-bearing catalog entry puts a live secret in `env`
(`SLACK_MCP_XOXP_TOKEN`, `EXA_API_KEY`), and those land in plaintext in each
assigned agent's global config file (`~/.claude.json`, `~/.codex/config.toml`, …)
because Vibe Kanban is a config writer, not an MCP client. The encrypted shared
gateway that would avoid this is streamable-HTTP only; a stdio server cannot use
it.

Gmail's entry carries a **filesystem path**, not a token. The refresh token stays
in `~/.gmail-mcp/credentials-*.json` under the Gmail server's own ownership and
never enters any agent config file. This is a genuine improvement over the Slack
shape, and it is recorded so that a later change does not "helpfully" inline the
token for convenience.
