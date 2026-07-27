# Data model: pinned-artifact audit incident

No Vibe Kanban database or API model changes.

## AuditIncident

A GitHub issue is the durable representation of an unresolved outer-launcher
digest failure.

| Field | Source | Meaning |
| --- | --- | --- |
| identity | fixed issue title and label | Deduplicates repeated scheduled failures |
| state | GitHub issue open/closed | Open means maintainers have not resolved the incident |
| body | workflow-created Markdown | Explains the expected control and first failing run |
| comments | subsequent failing runs | Records repeat observations and run URLs |
| labels | fixed supply-chain/audit label where available | Makes the incident searchable |

## Lifecycle

1. Digest matches: no issue mutation.
2. Digest fails and no matching open issue exists: create it.
3. Digest fails and a matching open issue exists: comment with the new run.
4. Maintainer investigates, publishes a new `-vk.<n+1>` release if required,
   updates the pin/digest/docs together, and closes the issue.
5. A later failure creates a new open incident if the prior issue is closed.

The workflow never auto-closes incidents: a subsequent green run does not prove
that users were unaffected or that the pin update process completed.

