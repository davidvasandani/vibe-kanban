# GitHub App organization credentials

## Problem
Organization PAT routing works, but a configured credential may not expose check runs from GitHub Apps. Add installation credentials so PR inspection and merge checks can use Checks read permission without requiring a classic repo token. Preserve truthful partial-source reporting.

## Contract
- Existing PAT entries retain their serialized form and behavior. An entry can instead select GitHub App authentication with App ID, installation ID, and a private-key 1Password reference. Never persist the resolved key or minted token in settings.
- Resolve App credentials centrally for server git and PR operations and workspace owner-routed gh/git commands. Authenticate with a short-lived signed JWT, verify the installation belongs to the configured owner, and mint installation tokens on demand. Cache by credential identity until shortly before GitHub's returned expiry, bound concurrent minting, and invalidate changed settings.
- Long-lived agent processes must refresh credentials when commands run; a launch-time token alone is insufficient. Configured failures must fail closed with the existing attributed vk_reported error instead of falling back to another identity.
- Settings reports separate evidence for App reachability, installation lookup, token minting, and effective required permissions. Failed stages are actionable and secret-free. Health is not proof of access to every repository.
- Do not log keys, JWTs, tokens, subprocess secret output, or API response bodies that may contain secrets.
- Keep get_pr/list_pr_checks aggregation semantics and merge checks intact. No new admin merge/bypass option in this task. Adding an App as a ruleset bypass actor is an external policy change and is not required for normal merging.

## Setup documentation
Document creating the App, installing it on selected organization repositories, storing its PEM key in 1Password, and switching the organization entry in Settings. Required repository permissions: Metadata read; Contents write; Pull requests write; Checks read; Commit statuses read; Actions read. Workflows write is optional for pushing workflow files. Explain expiry, rotation, health, repository selection, and PAT migration.

## Validation
Cover legacy configuration, malformed IDs/references/keys, owner mismatch, missing permissions, expiry/refresh and concurrent minting, settings changes, secret redaction, and routing in server and long-running workspace commands. Verify frontend types and formatting, run focused Rust and frontend checks, and independently review the diff. Live acceptance needs an installed App/key: get_pr on sweetgreen/ai-foundry must report check_runs ok and complete true when all sources succeed; list_pr_checks must include non-Actions check runs where present; push/create/merge must retain normal behavior. Record unavailable live prerequisites explicitly, never claim mock tests prove live acceptance.

## Scope
Only Vibe Kanban source and its deployment configuration in homelab/modules/vibe-kanban-rebuild.nix if needed. No modifications to other services or organization rulesets.
