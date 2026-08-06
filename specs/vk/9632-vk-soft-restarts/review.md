# Independent Review: Vibe Kanban Soft Restarts

The Vibe Kanban and homelab commits were reviewed independently with Codex CLI.
Confirmed findings were fixed and the affected verification was rerun.

## Confirmed findings addressed

- Scoped systemd signals to the worker main process so drain/resume signals do
  not reach coding-agent children in the service cgroup.
- Made active-execution evidence authoritative across admission races and state
  reads, including live processes whose protocol record was quarantined.
- Made the persisted drain marker fail closed on filesystem metadata errors.
- Corrected legacy-worker capability detection so JSON `false` is recognized as
  a present `admission_draining` field.
- Removed the incomplete PTY-reconnect experiment after review showed that true
  terminal resilience requires an acknowledged journal; PTYs are now explicit
  future scope rather than an unsupported continuity claim.

## Result

- Homelab commit review: no significant findings.
- Vibe Kanban commit review: no significant findings after the
  quarantined-process fix and regression test.
