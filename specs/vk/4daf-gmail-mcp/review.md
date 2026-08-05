# Independent review: Gmail MCP connector (`vk/4daf-gmail-mcp`)

## How this review was run — and what it was not

The pipeline calls for a **Codex** review. Codex CLI 0.146.0 installs and runs in
this environment, but has **no OpenAI credentials**: every request returned
`401 Unauthorized: Missing bearer or basic authentication in header`. A Codex
review was therefore **not performed**, and nothing below should be read as one.

Substituted: two independent reviewers with fresh context and no knowledge of the
spec's reasoning, each given one half of the diff and told to hunt for real
defects with concrete failure scenarios. That is an independent adversarial
review, but it is the same model family as the author, so it is weaker evidence
than a cross-vendor review would have been. Worth re-running under Codex before
merge if credentials are available.

Every finding below was re-verified directly against the code before acting on
it; two claims that could not be confirmed were checked empirically.

---

## Fixed

### F1 — Conflicting server names were invisible to name allocation *(destructive)*

`McpSettingsSection.tsx` allocated against `draft.servers` only. The backend puts
a name in `servers` **XOR** `conflicts` (`reconcile_snapshots`), and
`draftFromSharedRead` preserves that split — **verified** by reading it: the
draft's `servers` array is built solely from `response.servers`.

Failure: a user with a clean `gmail` plus a conflicted `gmail_2` (divergent
definitions across two agents) clicks the Gmail tile. Allocation returns
`gmail_2`, colliding with the unresolved conflict. On save,
`plan_servers_for_executor` removes that name from every executor not in the new
server's assignments — and `addPreconfigured` assigns only one
(`.slice(0, 1)`) — so the other agent's hand-edited entry is **deleted**, with no
conflict prompt.

Fixed by adding an exported `takenServerNames(draft)` that unions both lists, and
using it at both allocation sites. A regression test asserts a conflicting
`gmail_2` pushes allocation to `gmail_3`.

### F2 — The rename path had the same omission *(pre-existing)*

`openDialog` passed `draft.servers` as `existingNames`, so the dialog's
duplicate-name validation could not see conflict names either — a user could
rename a server onto a conflicting name and hit the same destructive path.

Not introduced here, but the same defect class one function away from the one
being fixed. Now uses `takenServerNames` too.

### F3 — `~` is never expanded, and the docs told users to type it *(data-loss risk)*

The entry shipped `GMAIL_CREDENTIALS_PATH: "YOUR_CREDENTIALS_PATH"` and the docs'
worked example used `~/.gmail-mcp/credentials-personal.json`.

**Verified empirically**: the built server contains **zero** occurrences of `~`
and no tilde-expansion code; `os.homedir()` appears only in its default
`CONFIG_DIR`. Env values are copied verbatim into agent-native config and the
server is spawned without a shell, so the tilde is never expanded — the path
would resolve against the agent's working directory, which is a task worktree.

Failure: a literal `~` directory inside the user's repository, containing a Gmail
**refresh token**, positioned to be committed by an agent.

The docs also contradicted themselves: the `auth` snippet runs through bash,
where `~` *does* expand, so the two halves produced different paths.

Fixed: placeholder is now `/absolute/path/to/credentials.json`, the worked table
uses absolute paths, and a `<Warning>` states that `~` is not expanded and why.

### F4 — The tool-prefix placeholder did not model its own trailing separator

Shipped `--tool-prefix=YOUR_TOOL_PREFIX` while every doc example used
`personal_`. A user mirroring the placeholder's shape gets `personalsearch_emails`
instead of `personal_search_emails`. Placeholder is now `YOUR_PREFIX_`, with the
reason recorded at the assertion site and in the docs warning.

### F5 — The integrity claim was overstated

The code comment, `AGENTS.md`, and the docs all argued the commit SHA makes this
pin *stronger* than Slack's. That is wrong in scope: a `github:` install runs the
package's `prepare` script, which resolves its **dependency closure from npm at
install time**. What executes is therefore less reproducible than Slack's
statically linked, digest-checked binary.

The conclusion (no digest constant, no audit job) still holds — auditing an
immutable pin is a no-op — but the justification now says only that, and states
explicitly that the SHA pins *source*, not dependencies. Corrected in all four
places.

### F6 — Latent `swap_remove` ordering trap, now adjacent to new code

`preserve_order` is enabled workspace-wide, which makes `Map::remove` in
`extract_meta` a **swap**-remove: it moves the map's last entry into the vacated
slot. This is harmless only because `meta` is the last key in
`default_mcp.json`. Adding `gmail` immediately before `meta` was correct, but
nothing said so — the next entry appended *after* `meta` would silently take
`meta`'s position in every generated agent config.

Pre-existing and not triggered by this change, but this change places a new key
right at the boundary. Documented at `extract_meta` with the rule for future
entries.

### F7 — Over-strict pin check with a misleading message

A 40-character **uppercase** SHA is equally immutable but failed, and the user
saw "an abbreviated SHA or a tag can be re-pointed" — neither of which applied.
Kept the lowercase requirement (one canonical spelling to compare against) and
rewrote the message to say that.

---

## Confirmed sound — no change

- **`nextAvailableServerName` itself.** Checked against a key already ending in
  `_2` (`gmail_2` + taken → `gmail_2_2`, still valid, and the two sequences can
  never converge because the loop is membership-driven), case-differing names
  (`Gmail` and `gmail` legitimately coexist — the backend is case-sensitive
  throughout), and termination.
- **The dependency array and rapid double-clicks.** `addPreconfigured` is rebuilt
  every render anyway (`profiles` is a fresh array each time), and React 18
  drains the sync lane before the next click task, so two clicks yield two
  servers.
- **Removing `disabled={added}`.** Nothing else read the disabled state; the
  `cn()` rewrite preserves dimming and makes hover unconditional, which is now
  correct since the tile is always actionable.
- **The immutability shape test.** Nineteen mutable spec variants were enumerated
  against its four gates; every realistic one fails. The only theoretical bypass
  is a branch named with exactly 40 lowercase hex characters, which git would
  resolve as an object id anyway.
- **Catalog iteration.** No test asserts catalog counts or key sets; every `meta`
  skipper still works; `adapt_codex`'s meta pruning keeps `meta.gmail` because
  the entry is stdio. JSON verified valid and byte-identical to a re-serialise.
- **No other code assumes server name == catalog key.** Test-result keys use
  `::`, excluded by the identifier charset, so suffixed names cannot alias.

## Noted, deliberately not fixed

- The `added` flag still compares against `server.key`, so it goes false once the
  user renames the first instance. Purely cosmetic now that nothing is gated on
  it; fixing it properly needs provenance tracking the data model does not have.
- "Already added" is now conveyed only by opacity and an unlabelled check icon;
  `disabled` previously exposed it to assistive tech. Pre-existing icon labelling
  gap, wider than this change.
- `canonical_definition_for_server` gates the legacy-Slack migration on the
  literal name `"slack"`, so a hand-copied legacy entry named `slack_2` is not
  migrated. Unreachable from the tile; out of scope.
- `mcpStrategies.ts:addPreconfiguredToConfig` writes a catalog key straight in as
  a config key, but it is dead code with no callers.
- The forbidden-substring loop in the pin test adds no coverage the hex check
  does not already provide. Harmless, and it keeps the test readable next to its
  Slack sibling.

---

## Verification after fixes

| Gate | Result |
| --- | --- |
| `cargo test -p executors` | 175 passed, 0 failed |
| `pnpm --filter @vibe/web-core test` | 242 passed (was 241; +1 regression test) |
| `pnpm run web-core:check` (tsc) | clean |
| `pnpm run local-web:lint` (eslint) | clean |
| `cargo clippy -p executors --all-targets` | clean |
| `pnpm run format` | applied |
