# Feature Specification: Markdown tables render as written

**Task**: `vk/37ef-fix-tables`
**Status**: Draft
**Constitution**: LVIII (new), XLVII, II, IV

## Summary

Agents often report results as GFM tables. In Vibe Kanban's markdown
surfaces (conversation messages, issue descriptions, comments, notes) those
tables render wrongly: the `|---|---|` divider line shows up as a row of
literal dashes, the header row is never styled as a header, and an empty
cell disappears so every cell after it slides one column left. The result
is misleading: in the reported case, two store names sat over the wrong
columns of a comparison table. This feature makes tables render the way the
author wrote them.

## User Stories

- **US1 (P1)**: As a user reading an agent's message that contains a
  comparison table with an empty top-left cell, I see each column heading
  above its own column, so I can trust which value belongs to which item.
- **US2 (P1)**: As a user reading any markdown table, I see the first row
  as a header and never see the divider line as a row of dashes.
- **US3 (P2)**: As a user writing or editing an issue description that
  contains a table, saving it keeps the same table: empty cells, pipes in
  cell text and the header survive a round trip unchanged in structure.

## Functional Requirements

- **FR-001**: A divider line written in any common GFM form (`|---|`,
  `| --- |`, `|:---|`, `| :---: |`, extra spaces, trailing whitespace) MUST
  NOT render as a table row.
- **FR-002**: When a divider line directly follows a table's first row, that
  first row MUST render as a header row. A divider anywhere else MUST be
  dropped without changing the table.
- **FR-003**: Empty cells MUST be kept in their position (first, middle or
  last), so every cell stays in the column its author put it in.
- **FR-004**: A body row with fewer cells than the header MUST be padded
  with empty cells. A row with more cells MUST keep all of them.
- **FR-005**: An escaped pipe (`\|`) inside a cell MUST stay in that cell and
  display as `|`.
- **FR-006**: Saving a table back to markdown MUST produce GFM that renders
  to the same table (same rows, columns, header, cell text).
- **FR-007**: Tables that already rendered correctly (no empty cells) MUST
  render the same cell content as before.

## Out of Scope

- Column alignment styling from `:---:` markers.
- Table editing controls (insert/delete rows or columns).
- The separate react-markdown preview, which already renders GFM tables
  correctly.

## Acceptance Criteria

- **AC-1** (US1, FR-002, FR-003): The reported table
  (`| | **22: …** | **23: …** |` + divider + body rows) renders with three
  header cells, the first empty, and no dashes row.
- **AC-2** (US2, FR-001): Each divider form in FR-001 is removed and marks
  the first row as header.
- **AC-3** (FR-003, FR-004): Rows with empty leading, middle and trailing
  cells keep their column positions; a short row is padded to the header
  width.
- **AC-4** (FR-005): `| a \| b | c |` renders two cells, the first being
  `a | b`.
- **AC-5** (US3, FR-006): Importing, exporting and re-importing each table
  above yields the same structure and header state.
- **AC-6** (FR-007): A plain table with no empty cells renders identical
  cell text to before.

## Clarifications

### Session 2026-10-21

- Q: What happens to a divider line that is not directly after the table's
  first row? → A: It is dropped and changes nothing else: no new table, no
  header change. Agents occasionally repeat a divider by mistake; dropping
  it never shows dashes (FR-001) and never splits a table the author meant
  as one. (Added to FR-002.)
- Q: Is a table with no divider still a table? → A: Yes, as today: rows
  render as a table with no header row. Changing that would break
  existing content that relies on it (FR-007).
- Q: Must export be byte-identical to the source? → A: No. Export must
  re-import to the same structure (FR-006). Spacing is normalised to
  `| a | b |` and dividers to `| --- |`, as before.

- Q: What does saving a table without a divider produce? → A: GFM with a
  divider, as the export already did, so the first row becomes the header
  from then on. Saved content stays valid GFM for agents and other
  renderers; the normalisation happens once and is stable afterwards.
  (Found during implementation; FR-006 holds for every table that has a
  divider.)

## Open Questions

- None.
