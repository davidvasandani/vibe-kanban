# Clarifications: automatic capacity backoff

No operator answers were available at this stage. Resolutions and their basis:

| Question | Resolution | Basis |
| --- | --- | --- |
| Literal script, or server-owned schedule? | Server-owned durable schedule | Goal is automatic resume; a script cannot survive restart or respect session admission (LX, LVII) |
| Enabled by default? | Yes, with a Settings toggle | Task wording "should automatically"; LX distinguishes this from XLI's opt-in |
| Switch model on retry? | No | Model changes need consent; out of scope in SPEC.md |
| Which turns count? | Completed or Failed turns whose log ends with the error entry | Codex records the errored turn as Completed |
| What resets the attempt count? | Any capacity failure of a turn not started by a retry | Keeps the bound per outage, with no extra state |

Remaining open questions: none.
