# Prior knowledge: automatic capacity backoff

Read-only search of `wiki/`. The knowledge base is not empty; five pages bear
on this task. Nothing in it covers capacity errors or automatic retries of a
provider failure.

## durable-session-wakeups.md
- A wake-up is a `wakeups` row plus an `issue_comment_deliveries` row with a
  nil issue id. The shared dispatcher delivers it: session admission lock,
  busy-turn waiting, user-queue priority, normal follow-up path, restart
  survival with lateness reported.
- Limits are 8 pending and 32 lifetime per session, to bound agent
  self-chaining. `Wakeup::insert` also unarchives the workspace.
- Cancellation and the dispatcher's claim compete on the same pending-state
  transition, so a successful cancel can never launch later.
- Stored prompts are untrusted for delivery markers: quote and escape.
- Startup wiring lives only in `startup::initialize_deployment`;
  `crates/server/tests/startup_wiring.rs` pins it. Subscribe before orphan
  cleanup and resume.

Implication: reuse this ledger for retries, but give capacity retries their
own insert that keeps its own bound, leaves the agent budget alone and never
unarchives.

## issue-comment-delivery.md
- The turn boundary is recorded by `finalize_task` in
  `session_turn_boundaries`, never guessed from time. Do not identify the turn
  in an asynchronous event handler from the event alone: a delayed event for
  turn A would confirm turn B.
- An execution row is not launch evidence. Evidence is a `pgid`, an accepted
  worker job or the agent's session id. An uncertain worker launch is neither
  delivered nor retried automatically.
- Implicit delivery never resumes a failed or stopped turn
  (`allow_stopped_target`).
- Busy means: a running coding agent, a queued message, or any non-persistent
  process in the workspace.

Implication: the detector must confirm that the recorded boundary names the
session's latest turn before reading it. A retry may resume a Failed turn only
after proving that turn is the one that hit capacity, without widening the
uncertain-launch rule.

## auto-error-remediation.md
- `finalize_task` only emits events; consumers in `server` do the I/O and read
  config when they act, so a toggle needs no restart.
- A broadcast with no receiver is lost: subscribe right after deployment
  creation.
- Reserve or record before I/O so a half-failed action is not repeated.
- Settings gotcha: `updateDraft` merges arrays by index. Scalars are safe.
- Remediation fires on `CodingAgent && Failed`. A Failed capacity turn would
  file an `Auto-fix:` issue for an upstream outage unless excluded.

## agent-process-lifecycle.md
- Codex is signal-driven (`exit_signal: Some`). Its turn end is reported over
  the wire and the reader loop signals Success, so an errored Codex turn is
  recorded Completed. Natural-exit executors (Claude and others) classify from
  the OS exit status.
- Worker reconciliation also converges on `finalize_task`.

Implication: trigger on the turn's error entries for both Completed and
Failed, not on status.

## session-chat-history-search.md
- `ContainerService::normalized_entries` returns `Option`. `None` means the
  log could not be read, which is different from "read, no errors". Treat it
  as no evidence and schedule nothing.

## host-env-sensitive-tests.md
- Some Slack MCP and GitHub-routing tests fail only on the agent host because
  of real environment variables. Re-run with them unset before blaming a diff.
