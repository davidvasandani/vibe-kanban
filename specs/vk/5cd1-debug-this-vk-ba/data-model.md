# Data model: reliable Claude background-Bash denial

No persistent, API, or generated data model changes are required.

The implementation has only transient protocol state:

- `grace_deadline: Option<Instant>` — absent before a real terminal result;
  after the result, the time at which a full quiet interval has elapsed.
- `spurious_fallback: Option<Instant>` — existing independent deadline for a
  non-error zero-turn resume artifact.
- `interrupt_sent: bool` — existing cancellation state.

The two deadlines remain mutually prioritized as in the current loop: terminal
grace supersedes the spurious-result fallback.
