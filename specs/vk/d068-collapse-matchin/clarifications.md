# Clarifications — vk/d068-collapse-matchin

No answers were supplied with `/speckit.clarify`. Both questions were resolved
from the task's screenshot and from the existing UI conventions:

1. **Group by name, or by name + arguments?** By name only. The visible
   duplication is the repeated `mcp:<server>:<tool>` label. Splitting on
   arguments would leave the screenshot's rows uncollapsed whenever the
   arguments differ. Per-call argument previews keep the calls distinguishable.
2. **Icon?** Reuse the generic tool icon (`ListMagnifyingGlassIcon`), which
   `ChatToolSummary` already uses for single `tool` rows.
