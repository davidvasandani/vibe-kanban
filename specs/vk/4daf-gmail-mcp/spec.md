# Feature Specification: Gmail MCP connector with multi-account instances

**Feature dir**: `specs/vk/4daf-gmail-mcp/`
**Status**: Draft

## Summary

Vibe Kanban users can connect their coding agents to Gmail by picking a bundled
**Gmail** entry from the popular-servers catalog, and can add that entry more
than once so a single user can work across several mailboxes (personal plus one
or more work accounts) in the same session. Today the catalog offers no Gmail
option at all, and — for any template — a second copy cannot be added, because a
template tile disables itself once one instance exists.

## User Stories

- As a Vibe Kanban user, I want to add Gmail to my agent from the popular-servers
  list so that I do not have to hand-write a server definition and hunt for the
  right install command.
- As someone with a personal mailbox and two work mailboxes, I want three
  independent Gmail connections active at once so that an agent can find a thread
  in one account and draft a reply from another without me reconfiguring
  anything.
- As that same user, I want each connection's tools to be distinguishable so that
  I can tell the agent which mailbox to act on, and so that one connection cannot
  silently answer for another.
- As a user of any other bundled template (Slack, Context7, …), I want to add it
  twice when I have two workspaces or two API keys, for the same reason.
- As a security-conscious user, I want to know that the thing being launched on
  my machine comes from the repository the catalog says it does, at a revision
  that cannot change underneath me.
- As a new user who has not yet set up Google credentials, I want the failure to
  tell me what is missing rather than looking like a broken product.

## Functional Requirements

**Catalog entry**

- FR-1: The bundled catalog offers a Gmail entry, displayed with a name and a
  one-line description, alongside the existing entries.
- FR-2: The entry launches the Gmail MCP server from the repository named in the
  entry's own metadata link, at a fixed revision that cannot be changed without
  changing the entry.
- FR-3: The entry ships a placeholder for every value that must differ between
  two instances, and no live secret. A user who adds it without editing must be
  able to see which values they are required to supply.
- FR-4: The entry does not require the user to supply a value that is correctly
  shared across all of their instances.
- FR-5: The entry works on every coding agent Vibe Kanban supports for stdio MCP
  servers, in that agent's own configuration shape.

**Multiple instances**

- FR-6: A user can add the same catalog template more than once. Each add
  produces a separate logical server, and adding a second one never replaces,
  merges with, or silently modifies the first.
- FR-7: Each instance is created with an identifier that the system will accept
  when saved, without the user having to correct it.
- FR-8: A user can rename any instance to something meaningful to them, within
  the identifier rules the product already enforces.
- FR-9: Each instance can carry its own credentials and its own tool-name
  disambiguator, independently of the others.
- FR-10: When several instances of one tool are assigned to the same agent, the
  agent can address each instance's tools distinctly.

**Setup and failure reporting**

- FR-11: The documentation states the prerequisites before a user adds the entry,
  distinguishing the one-off account-independent setup from the per-mailbox step
  that must be repeated for each instance.
- FR-12: When a prerequisite is missing, the existing connection test reports a
  failure carrying the underlying tool's own explanation of what is absent.
- FR-13: The documentation states what goes wrong if a user gives two instances
  the same tool-name disambiguator, because that failure is silent rather than
  loud.

**Provenance**

- FR-14: An automated check fails if the entry's install source is ever changed
  to something mutable, or to a repository other than the one its metadata links
  to.
- FR-15: The revision pin, and every document that names it, are changed together
  or not at all.

## Out of Scope

- Vibe Kanban performing the Google OAuth consent flow. The user completes it
  once per mailbox using the Gmail server's own command.
- Storing Gmail credentials in Vibe Kanban, encrypted or otherwise. Only a path
  to the user's own credential file is configured.
- Any Gmail-specific user interface — no mailbox picker, no account list, no
  message rendering.
- Shipping the requester's specific mailbox labels ("Sweetgreen", "Proalign") as
  catalog entries. Instance names are chosen by each user.
- A human-readable display label distinct from the protocol identifier. There is
  nowhere to persist one, since agent-native configuration stores only the
  identifier.
- Changes to how logical servers are assigned to agents, tested, reconciled, or
  written into agent-native configuration.
- Automated dependency updates for this entry. The revision is bumped by hand and
  that is stated where maintainers will look.

## Acceptance Criteria

- [ ] A Gmail tile appears in the popular-servers list with its name and
      description.
- [ ] Clicking the Gmail tile once produces one Gmail logical server; clicking it
      again produces a **second, distinct** logical server, and the first is
      unchanged.
- [ ] The automatically assigned name of a second instance is accepted by a save
      without the user editing it, and without a validation warning.
- [ ] Both instances can be renamed, given different credential paths and
      different tool-name disambiguators, saved, and re-read with those values
      intact.
- [ ] After saving two instances assigned to one agent, that agent's own
      configuration file contains two distinct entries.
- [ ] Installing the pinned revision on a machine with no prior cache produces a
      runnable server. *(Verified: 53 s cold install, executable `dist/index.js`
      produced.)*
- [ ] Driving the installed server over its protocol returns its tool list, with
      every tool name carrying the configured disambiguator. *(Verified: 28 tools
      returned, all prefixed.)*
- [ ] With the disambiguator set differently on two instances, the two tool sets
      do not collide.
- [ ] Running the automated provenance check against an install source pointing
      at a branch, at `@latest`, or at a different repository than the metadata
      link **fails**.
- [ ] Adding the entry with no Google OAuth client present produces a connection
      test failure whose text names the missing file. *(Verified: the server
      exits before completing its handshake and reports the missing file by
      name.)*
- [ ] The existing catalog entries' behaviour is unchanged: their tiles still add
      a server, and their existing automated checks still pass.
- [ ] Repository verification (`cargo test`, frontend tests, `check`, `lint`,
      `format`) passes.

## Open Questions

Resolved during specification; recorded here with their resolution so the
`clarify` stage does not reopen them.

- ~~[NEEDS CLARIFICATION: does "my fork" mean the fork must be the install
  source, given it is currently identical to upstream and unpublished?]~~ →
  Resolved: yes. The fork is the install source, pinned by commit SHA. It builds
  from a git checkout because the package declares a `prepare` script. The
  upstream npm package is recorded as the documented fallback.
- ~~[NEEDS CLARIFICATION: should the three named mailboxes be three catalog
  entries?]~~ → Resolved: no. One entry, instantiated three times and renamed by
  the user. Per-user rows in a shipped catalog would publish private affiliation.
- ~~[NEEDS CLARIFICATION: is a separate integrity digest and audit job required,
  matching the Slack entry?]~~ → Resolved: no. A commit SHA is content-addressed
  and verified at install time; the Slack audit exists only because a release
  asset is mutable under a fixed tag.

Resolved during the clarify stage — see [`clarifications.md`](clarifications.md):

- ~~[NEEDS CLARIFICATION: should the cold-start build cost be reduced first?]~~ →
  Resolved (C4): no, ship the git SHA pin and accept the ~53 s first launch, with
  three named conditions that reopen the decision.
- ~~[NEEDS CLARIFICATION: should a Gmail icon ship?]~~ → Resolved (C5): no. The
  tile falls back to an initial, as Slack's already does; the logo is a trademark
  question, and adding one later is a one-line change.

**No open questions block implementation.** One preference is referred to the
requester rather than decided in a spec file: whether the ~53 s first launch is
acceptable, or whether they would prefer the upstream npm package for speed at
the cost of not being their fork. Reversing C1 is a one-line change to the entry,
its test constant, and the docs.
