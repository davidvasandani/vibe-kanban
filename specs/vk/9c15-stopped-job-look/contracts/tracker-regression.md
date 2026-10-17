# Contract: worker event tracker and journal regression

Invariant: for a live worker journal, every value the tracker holds as
`cursor` is a sequence that journal served, so
`batch.latest_available >= cursor`.

| Observation on a successful `events(after = cursor)` | Tracker action |
| --- | --- |
| `latest_available >= cursor` | unchanged behaviour (process events, ack) |
| `replay_gap` error | unchanged replay-gap behaviour |
| `latest_available < cursor`, DB + inventory readable, exact-identity terminal summary with `last_sequence == latest_available` | mark output incomplete, stderr notice, finalize through the normal terminal block with the summary's state |
| `latest_available < cursor`, lookups readable, no such summary | mark output incomplete, notice, `Indeterminate`, finalize |
| `latest_available < cursor`, DB or inventory lookup fails | warn, back off, re-poll; never infer |
