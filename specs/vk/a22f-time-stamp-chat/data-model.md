# Data Model: Timestamp the Workspace Chat Log

No persisted data-model or API changes are required.

## Existing source values

- `NormalizedEntry.timestamp: string | null`: authoritative timestamp for a
  normalized message/event/action when supplied by the executor normalizer.
- `ExecutionProcessStaticInfo.created_at: string`: authoritative creation time
  used by client-derived entries representing the process prompt or script.
- `DisplayEntry`: either one keyed patch or an aggregate containing ordered
  keyed patches.

## Derived presentation value

`ConversationTimestamp` is an ephemeral view value:

- `isoTimestamp`: validated original timestamp string.
- `compactLabel`: localized time, with short date when not today locally.
- `fullLabel`: localized full date and time for title/accessible description.

For aggregates, select the member with the greatest valid epoch time. Missing
or invalid source values derive no presentation value.
