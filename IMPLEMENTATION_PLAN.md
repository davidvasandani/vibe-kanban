# Implementation plan: session execution exclusivity

1. Complete SpecKit stages in order, using the current task's feature directory and Vibe Kanban constitution. Carry workspace PRIOR_KNOWLEDGE.md findings into the feature plan.
2. Trace routes/sessions follow-up and queue admission, MCP run_session_prompt, issue_comment_delivery, ContainerService start_execution and exit-monitor handoffs. Establish a shared admission boundary before async startup and preserve queued message ownership.
3. Enforce coding execution exclusivity in the DB/server launch path, including initial and automatic turns. Add delayed-start and concurrent request tests with deterministic synchronization.
4. Guard worker execution admission by session while startup or a child remains alive; retain execution-ID replay idempotency and cleanup semantics. Test refusal and subsequent admission after reaping.
5. Inspect Claude peer configuration. Disable unsafe native peer messaging if isolation is unsupported; use deterministic workspace/session identity for supported VK attribution. Test permission modes and document identity and same-worktree collision guidance.
6. Install dependencies, run focused tests and required formatting/checks, resolve failures, and obtain independent Codex diff review until significant findings are cleared.
7. Update and commit wiki knowledge tagged vk/0890-investigate-a-fo, open a PR against the base branch, observe required checks, and merge. Report any unverified live behavior accurately.
