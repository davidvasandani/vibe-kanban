# Contract: Codex Normalized Patch Stream

For an eligible uninterrupted successful run:

1. The first command start emits `add /entries/{i}` with unchanged content.
2. Updates and completion for that call emit `replace /entries/{i}`.
3. Each later command start emits `replace /entries/{i}` while retaining only
   the marker for already successful occurrences; it does not allocate another
   entry index.
4. Successful completion adds the latest bounded repeat marker.

The shared row's metadata identifies the latest owning call ID.

A different allocated visible entry or changed command prevents reuse. If a
reused owner fails, the prior successful aggregate is restored and the failure
is added at a fresh index; the next matching command starts a new run.

Commands outside `codex review --uncommitted` retain the existing contract of
one allocated normalized entry per execution.
