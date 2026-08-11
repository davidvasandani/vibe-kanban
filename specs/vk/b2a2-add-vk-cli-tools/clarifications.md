# Clarifications: CLI Tools in Workspace Sessions

**Task**: `vk/b2a2-add-vk-cli-tools`
**Status**: Resolved from the request, screenshot, repository behavior, and
project knowledge

## C1 — Which workspace process boundaries are in scope?

Both coding-agent execution and interactive workspace terminals are in scope.

The request says “workspace sessions,” and the existing project knowledge warns
that “available in the workspace” spans managed execution and the independently
spawned PTY path. The managed execution path already attempts to append the CLI
tools bin directory, while the terminal path currently receives only resolved
organization variables. Treating only agents as in scope would preserve the
reported inconsistency for users entering the same workspace through its
terminal.

This does not pull managed CLI login terminals into scope: they are
machine-scoped settings sessions, not workspace sessions, and intentionally use
a minimal allowlisted host environment.

## C2 — Must this feature distribute CLI installs to cluster workers?

No. Installation/synchronization across machines is out of scope. Each process
spawn uses a managed tools directory only when that directory is valid and
available on the host that actually executes the process.

The CLI Tools UI is machine-scoped, and cluster workers have their own service
state and process ownership. The repository provides no contract that the
coordinator's application-data directory is node-identical shared storage.
Sending a coordinator-local absolute path to a worker would violate the
constitution's cross-node path rule. A worker may expose its own node-local
managed directory if present; otherwise it preserves its existing PATH and
starts normally.

Provisioning, copying, or mounting managed installs across workers would change
deployment and lifecycle semantics substantially beyond “add tools to PATH.”

## C3 — When does a session adopt a newly installed tool?

At process creation. Already-running shells and agent processes are unchanged.

Environment variables are captured when a child process starts. Mutating an
existing process would require shell-specific command injection or process
restart behavior that the request does not ask for. A user starts a new
workspace process after installing a tool to receive the new PATH contract.

## Remaining Open Questions

None.
