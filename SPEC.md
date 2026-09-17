# AWS SSO and CLI availability in agent shells

Task: vk/c817-aws-sso-sign-in

## Problem
AWS Settings authenticates the service host and CLI Tools discovers host binaries, while agent turns may run with an isolated home and store on another worker. Success in Settings therefore does not establish usable agent credentials or tools.

## Required behavior
- Make the AWS configuration and short-lived SSO session established in Settings available to agent turns without copying secrets into conversation, logs, or version control.
- Preserve sandbox isolation outside the intended AWS state. Account for worker affinity, missing state, token refresh, and subsequent turns.
- Provide AWS CLI on the actual agent PATH, including its runtime closure where Nix store views differ.
- CLI Tools must distinguish host availability from verified agent availability; never imply remote worker reachability from a host probe.
- AWS status must clearly identify its scope and distinguish a timed-out check from successful agent authentication.

## Proposed approach to validate during planning
Trace Settings host selection and executor home/environment construction. Prefer exposing the intended service AWS state at the agent home using the existing sandbox mechanisms. Use the CLI-managed AWS storage layout so both CLI and Node SDK default providers find SSO state; do not invent an unsupported cache environment variable. Ensure installed tools reach the executor environment, or explicitly label them host-only. Scope infrastructure changes to Vibe Kanban deployment.

## Acceptance and verification
Regression coverage must exercise isolated HOME, absent AWS state, refreshed state on later turns, tool path visibility, and truthful Settings labels. Where an authenticated live session is available, verify both aws sts get-caller-identity and a Node SDK default credential provider from an agent shell without exposing credentials. Report live verification limitations honestly. Run repository checks and independent Codex review; record reusable knowledge, then open and merge PRs against the base branch.

## Out of scope
Other services, static access-key provisioning, IAM permission changes, and the related MCP connection defect.
