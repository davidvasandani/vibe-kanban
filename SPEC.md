# Session step timestamps

Task: `vk/77a2-session-step-tim`.

## Problem and scope
The supplied screenshot shows many distinct session events stamped 9:19 PM while the session remains active at 10:34 PM. Investigate whether steps incorrectly inherit the execution start time, lose their source timestamp, or freeze during streaming. Scope is the Vibe Kanban service repository; hosting changes only if evidence requires them.

## Required behavior
- Each session step displays the time of that event when available, consistently for streaming and reloaded history.
- Updating a step must preserve its original event timestamp unless the source provides a more authoritative timestamp.
- Missing timestamps must not be presented as precise event times derived from an unrelated execution start.
- Preserve chronological ordering, existing timezone formatting, and compatibility with historical logs.

## Approach and acceptance
Trace source logs through normalization, storage/stream transport, and frontend display. Fix the earliest incorrect transformation with targeted regression coverage. Verify distinct events within one execution retain distinct timestamps, updates remain stable, and legacy missing timestamps behave honestly. Run relevant checks and independent Codex review before opening and merging the PR. Record reusable findings in the project knowledge base.

## Resolved timestamp source
Fresh execution stores record the first server observation of an untimed
normalized step. Valid vendor time takes precedence; replacements retain the
recorded time. Historical normalization and adopted/reattached executions do
not acquire current-clock timestamps. Completed sidecars retain recorded times;
if only raw history survives, unavailable step times remain omitted. Timestamp
identity follows array insertion/removal, including AmpResume history resets.
