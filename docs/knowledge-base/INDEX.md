# Project knowledge base

Distilled, reusable knowledge from completed tasks. One topic per page; each page lists the
task ids that contributed to it. Consult this index before planning a new task; add or
update pages (and this index) when a task ships something reusable.

| Page | Summary | Contributing tasks |
| --- | --- | --- |
| [claude-log-normalization](claude-log-normalization.md) | How `ClaudeLogProcessor` turns stream-JSON into `/entries/{i}` patches; `EntryIndexProvider` idioms; the AmpResume index-reset gotcha | `4095-thinking-tokens` |
| [collapsing-repeated-log-entries](collapsing-repeated-log-entries.md) | Server-side pattern for collapsing uninterrupted repeated log events into one entry with a `✓` per repeat | `4095-thinking-tokens` |
