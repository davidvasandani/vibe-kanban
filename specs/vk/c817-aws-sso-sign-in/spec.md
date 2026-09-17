# Feature: AWS SSO and executable availability in agent shells
Task: vk/c817-aws-sso-sign-in
Command: /speckit.specify
Status: specified

## Summary
A Settings success must describe the environment it actually checked. The managed Vibe Kanban cluster must make Settings AWS logins usable in agent shells, with no pasted credentials.

## User stories
- As an operator, I sign in once and use that SSO profile in a worker agent to run AWS diagnostics.
- As an operator, I can distinguish host CLI availability from availability on a workspace worker.

## Functional requirements
- FR1: Service AWS profile and live SSO state are reachable by agent shells on configured cluster workers, including isolated homes.
- FR2: AWS CLI is available on worker PATH through the deployment's toolchain.
- FR3: CLI Settings labels the checked host scope for every available tool; it makes no unverified agent availability claim.
- FR4: AWS Settings identifies host-scoped authentication checks and explains profile selection in agents.
- FR5: Existing profile and session data is preserved during migration, secret values never appear in task artifacts, and missing state remains actionable.
- FR6: Later login and token refresh become visible without stale per-turn snapshots.

## Acceptance
- After Settings login, `AWS_PROFILE=<selected> aws sts get-caller-identity` and the Node SDK default provider chain succeed from a worker agent with that profile selected.
- Worker AWS CLI resolves from PATH; CLI Settings marks available entries as host-scoped unless agent reachability was verified.
- Regression tests cover absent initial state, isolated HOME, live refresh, and migration preservation.

## Out of scope
Other services; IAM changes; long-lived credential provisioning; the related MCP defect.

## Open questions
Resolved by /speckit.clarify:
- Reuse the deployment shared root and aligned service UID/GID. Coordinator state is authoritative; preserve existing local directories as backups and reject conflicting coordinator migration instead of overwriting files.
- Keep AWS vendor state writable for SSO refresh, just as the existing service credentials are; limit directory access to the service identity. This is an explicit deployment credential-sharing contract, not a universal claim about arbitrary remote workers.
- Select a named profile with AWS_PROFILE (or CLI --profile); do not choose one of 31 profiles automatically.
- Report host availability and agent availability as unverified in Settings unless actually probed. No new remote probing API is required for this task.
- No remaining open questions.

## Follow-up acceptance: reliable status checks
- At most four AWS auth probes execute concurrently per server process, including overlapping list and post-login requests.
- The execution budget is 15 seconds and starts after admission; admission waiting is capped at 30 seconds with a distinct busy message.
- Results remain attached to the correct profiles; cancellation frees capacity and terminates any active CLI subprocess.
- The 31-profile ai-foundry session reports authenticated after its successful sign-in. Live bounded reproduction passed all 31 with four concurrent probes.

## Follow-up: lazy profile admission

Live validation of #304 exposed that eagerly starting 31 admission deadlines
causes later profiles to expire behind their own batch. Admit at most four
profile futures per list while retaining the process-wide semaphore and
per-admission/per-execution deadlines. Preserve profile ordering. Resolve the
AWS executable once per refresh, under the same global capacity limit, using
a request-local async cell; do not retain stale executable paths across refreshes.
Verify overlapping 31-profile batches whose total duration exceeds 30 seconds,
including ordered identity results and the global process limit. Then run the
AWS tests, formatting, independent Codex review, knowledge update, and PR merge.
