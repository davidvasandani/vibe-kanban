# Clarifications

`/speckit.clarify` resolved all open questions using the task's stated need,
existing deployment conventions, and the project constitution.

## C1 — Credential source

**Decision:** Configure one 1Password item/field reference per GitHub owner and
resolve all PATs during service startup with the deployment's existing
runtime-only 1Password bootstrap credential.

**Reason:** The fleet already provisions that bootstrap through systemd
credentials. Supporting arbitrary token files as a second source expands the
configuration and validation surface without serving the described deployment.
PAT values still land only in a node-local runtime directory.

## C2 — Multiple Git remotes

**Decision:** When `--repo` is absent, resolve the remote in this order:
`remote.pushDefault`, current branch `pushRemote`, current branch `remote`,
`origin`, then a sole configured remote. If no unique candidate exists, leave
authentication unchanged.

**Reason:** This follows Git's explicit push/upstream intent before conventional
fallbacks and avoids silently choosing among several unrelated remotes.

## C3 — Ambient `GH_TOKEN`

**Decision:** A configured owner-specific PAT wins over an ambient `GH_TOKEN`.
For an unconfigured or unresolved owner, preserve the caller's environment.

**Reason:** The feature exists because one ambient GitHub.com token is
insufficient or wrong for multi-owner workspaces. Allowing it to override the
mapping would make routing nondeterministic, while preserving it on the fallback
path maintains existing behavior.

No blocking questions remain.
