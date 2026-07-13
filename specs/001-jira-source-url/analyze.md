# Analysis: spec ↔ plan ↔ tasks cross-check

**Task**: `vk/a793-vk-jira-bi-direc`

## Requirement coverage
| Req  | Covered by | Status |
|------|------------|--------|
| FR-1 (show key + URL in panel) | plan §Changes 1–2, T2/T3 | ✅ |
| FR-2 (new tab, panel preserved) | reuses `JiraBadge` (`target=_blank`, `e.stopPropagation()`) | ✅ |
| FR-3 (dormant/deleted de-emphasis) | `active = link_state==='active'` → `JiraBadge active` opacity | ✅ |
| FR-4 (no link → unchanged layout) | badge gated `!isCreateMode && jiraLink`, no wrapper/section | ✅ |
| FR-5 (single source of truth) | same `getJiraLinkForIssue` as card; shared prop shape + `JiraBadge` | ✅ |
| FR-6 (status sync unchanged) | no backend change; T5 keeps `mapping.rs` green | ✅ |

## Constitution check
- **I Clarity**: reuses an existing badge + prop shape; no new abstraction. ✅
- **II Test the contract**: T4 adds a rendered-DOM assertion; acceptance
  criteria are concrete. ✅
- **III Small/reversible**: one optional prop + one container line; additive. ✅
- **IV Shared-component boundary**: panel owns placement, container supplies
  data; blast radius (local+remote) acknowledged in plan §Risks. ✅
- **V Don't rebuild what shipped**: reconciler untouched; Req 2 asserted, not
  reimplemented. ✅

## Gaps / risks found
- None blocking. Minor: ensure the container passes `undefined` (not a fresh
  `{}`) when there's no link, and memoizes the object so the panel doesn't
  re-render on every parent render — captured in T3.
- Consistency note: card derives `active` as `link_state === 'active'`; the
  panel must use the identical derivation to avoid a mismatch between card and
  panel dimming (captured in plan research notes).

**Verdict**: consistent and constitution-compliant. Cleared to implement.
