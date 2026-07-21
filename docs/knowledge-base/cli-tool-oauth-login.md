# Managed CLI tool OAuth login

Tags: `5a2a-vk-cli-tool-logi`, `6777-aws-sso-config-i`

## Boundary: orchestrate the vendor CLI

For managed command-line tools, Vibe Kanban should not become an OAuth client or
credential store. Launch the vendor's durable device/browser login command in a
PTY and leave tokens in the CLI's normal host-side storage. Only offer in-app
login when both conditions hold:

1. Credentials survive the login child process and are usable by later agents.
2. A separate, non-secret command can independently verify authentication.

That permits Azure CLI (`az login --use-device-code`, then `az account show
--output none`) and GAM (`gam oauth create`, then `gam oauth verify`). It rules
out generic AWS SSO because it is profile-specific, 1Password because sessions
may be shell-scoped, and the pinned Graph beta CLI because it has no independent
status command. AWS SSO later gained a profile-scoped flow outside the generic
catalog — a parallel `/api/aws/*` route set that keeps the same PTY/probe/lock
discipline with a runtime-chosen `--profile` (see
[aws-sso-profile-management](aws-sso-profile-management.md)); the generic
catalog contract stayed untouched.

## Backend pattern

- Keep executable and arguments in the compiled server catalog; never accept a
  command string from the browser.
- Resolve the effective binary exactly as agent execution does. In this project,
  a host binary takes precedence over the app-managed copy.
- Run status probes concurrently, with a short timeout, `kill_on_drop`, a
  minimal environment, and typed `authenticated`, `unauthenticated`, `unknown`,
  and `unsupported` results.
- Use the existing signed WebSocket and machine-aware routing. The frontend must
  pass the selected host/relay scope explicitly; a path alone can target the UI
  machine by mistake.
- Enforce one active login per tool in each server process and a maximum session
  duration. Stream PTY bytes only; do not persist or log transcripts.
- Treat command exit and authentication verification as distinct facts. A zero
  exit becomes success only after the independent probe confirms authentication.

## PTY lifecycle lesson

Direct command sessions need an exit channel and a cloned child killer. Cancel,
timeout, and disconnect terminate and remove the child. Normal completion is
different: once the waiter has reaped the child, remove the session without
signalling the cloned PID. Sending a Unix signal after `wait()` risks hitting an
unrelated process if the PID is rapidly reused.

The browser must also handle `error` and premature `close` events. A WebSocket
constructor can succeed before its HTTP upgrade is rejected, so promise
rejection alone is insufficient and otherwise leaves a terminal stuck in its
running state.

## Validation pattern

Cover catalog eligibility and concurrent locks without invoking real logins;
test a harmless direct PTY command for output and exit reporting; test frontend
action visibility as a pure state mapping. Run generated-type checks, focused
Rust tests/clippy, frontend tests/type checking, and an independent diff review.
