# Prior knowledge: GitHub App organization credentials

Read-only search of vibe-kanban/wiki, especially github-owner-token-routing.md and 1password-reference-resolution.md, found:

- Machine credentials live in github_owner_tokens, not Config, because /api/info exposes Config. Secrets use row-bound encryption and write-only literals; references may be returned.
- services/github_credentials resolves server git and PR credentials per request; utils/github_credentials applies them to a single command. Configured failures refuse execution. Never Debug a Command with credentials.
- Workspace gh routing and git credential helpers live in utils/github_auth. Worker and terminal boundaries also apply routing. Launch-time environment tokens currently have no refresh mechanism, so installation expiry requires a command-time resolver.
- Owner routing must preserve argument parsing, GitHub host isolation, case variants, URL rewriting, inherited header resets, and fail-closed behavior.
- The 1Password resolver supports organization-selected Connect then service-account fallback. Server lookups use credential files because deployment removes ambient secrets. Cache generation prevents in-flight lookups from restoring invalidated entries.
- Health uses the same resolver and logs transitions only. Existing deployment monitors depend on the health status vocabulary.
- 1Password errors must be classified without echoing stderr or reference names. PEM keys require a multiline-safe path; existing environment-value validation must be inspected before reuse.

These findings refine SPEC.md and the implementation plan. Admin bypass remains deferred. Live acceptance requires provisioned App credentials; no unrelated service or ruleset edits are authorized.
