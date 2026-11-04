# GitHub App organization credentials

Status: specified. Applies to Vibe Kanban only.

## User stories
- An administrator configures an organization installation without disclosing its private key to agents.
- An agent reads checks from all GitHub Apps and continues git/PR work after an installation token expires.
- An administrator diagnoses credential failures in Settings and can retain or restore PAT authentication.

## Requirements
FR-1: Support a PAT or App ID, installation ID and 1Password key reference per owner, preserving legacy PAT configuration.
FR-2: Use the selected identity for routed gh, pushes, fetches and every PR tool, including workers and terminals.
FR-3: Renew installation tokens on demand before expiry, avoid duplicate concurrent minting, and honor credential changes.
FR-4: Expose App reachability, installation ownership, token minting and effective permission health without secrets.
FR-5: Refuse configured credential failures, preserving attributed vk_reported errors and partial check coverage.
FR-6: Document installation on selected repositories and minimum permissions, including optional workflow writes.
FR-7: Preserve existing merge gates. Do not introduce an admin merge option or modify bypass actors.

## Acceptance
- Existing PAT CRUD and routing tests pass unchanged in behavior.
- Cached installation credentials refresh before expiry, including in a running workspace; mismatch and mint errors cannot fall back.
- Settings supports creating and replacing either credential kind and reports staged health.
- Automated tests cover cache concurrency/invalidation, malformed inputs, API failures and secret redaction.
- With a configured installation, get_pr reports check_runs ok and complete true when all sources succeed. list_pr_checks includes non-Actions check runs where present. Push/create/merge use the App identity.

## Open questions
None requiring user input. Clarification records the admin-bypass decision and live provisioning prerequisites.

## Clarifications
Applied `/speckit.clarify`.
- Admin merge is deferred, both per-repository and per-author. Normal GitHub rules still apply; do not add the App to ai-foundry ruleset 23498284 as part of this feature.
- App IDs and installation IDs are positive decimal strings, avoiding JavaScript numeric precision loss.
- Only a 1Password reference is accepted for the private key. No raw PEM field is returned to the browser or exported to agents.
- Health describes the credential and effective permissions; repository selection remains a separate access condition.
- Live tests require an existing installation and private-key reference. Missing provisioning is reported as unverified acceptance, not success.
