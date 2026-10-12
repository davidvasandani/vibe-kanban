# Clarifications — vk/2b39-fix-mobile-ui

No answers came with `/speckit.clarify`. Each question was resolved from the
task pipeline and the spec's user stories.

| # | Question | Resolution | Basis |
|---|----------|------------|-------|
| 1 | "Mock up some examples": mockups only, or ship a fix too? | Both. Check in mockups 0/A/B/C and ship A. | The pipeline requires implementation and a merged PR. The mockups let the user pick B or C later as a small follow-up. |
| 2 | On mobile, keep the second metadata line (A) or collapse it to counters (B)? | Keep it (A). | User story 2: blockers and tags must stay visible. Counters hide *which* issue blocks this one. |
| 3 | Metadata line too narrow: wrap or clip? | Clip on one line (`overflow-hidden`). | Wrapping makes row height depend on the data, so the list jumps around. Two line heights are predictable. LIII allows truncation. |
| 4 | Mobile breakpoint? | The existing `useIsMobile()` (`max-width: 767px`). | Constitution LIII and the workspace-context-bar wiki: use the layout signal, not UA detection. |
