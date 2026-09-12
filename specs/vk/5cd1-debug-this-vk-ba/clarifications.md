# Clarifications: reliable Claude background-Bash denial

**Spec**: `./spec.md`

## Decisions

1. **What closes the stream?** Vibe Kanban arms
   `POST_RESULT_GRACE` once and never refreshes it. Post-result stdout/control
   activity can therefore continue toward a hook request while VK's original
   timer expires and drops the last stdin owner. Anthropic SDK issue #369 and
   related reports independently confirm that result frames are not sufficient
   evidence that bidirectional hook traffic has ended.
2. **How will this be tested?** Refactor only the private read-loop boundary to
   accept generic Tokio `AsyncRead`/`AsyncWrite` values. Production still
   passes child pipes; tests pass a duplex transport and paused Tokio time.
3. **What ends the grace?** A full `POST_RESULT_GRACE` interval with no new
   Claude output after a terminal result. Every parsed or unparsed non-empty
   line is activity and refreshes the deadline. Control requests remain handled
   inline, so their response is flushed before the loop can re-evaluate the
   deadline.
4. **Do we remove the bound?** No. Claude's structured-input process may wait
   for stdin EOF, so an unbounded wait can strand every completed turn.
5. **Do we weaken the hook?** No. The background-Bash denial and
   `spawn_poller` redirect are required behavior.

## Remaining questions

None.
