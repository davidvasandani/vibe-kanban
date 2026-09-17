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

## Follow-up: reliable AWS authentication probes
A fresh sign-in succeeds but the status checker starts 31 AWS CLI subprocesses simultaneously and kills them after five seconds. Live reproduction: a single host probe passed in 1.23 seconds; 30 of 31 unbounded concurrent probes timed out. Four concurrent probes passed all 31 in 15.89 seconds, slowest 3.1 seconds.

Limit authentication probes to four per server process across list requests and post-login verification. Give admitted probes 15 seconds to finish. Bound admission waiting separately to 30 seconds, reporting busy capacity distinctly from an executed probe timing out. Preserve profile/result association, credential-environment isolation, failure classification, and kill-on-cancellation. Add regressions for overlapping batches, waiting budgets, timeout/cancellation permit release and status mapping.
