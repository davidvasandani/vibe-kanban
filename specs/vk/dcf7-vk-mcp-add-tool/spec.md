# Feature Specification: MCP Tool Count and Last-Checked Time

**Feature directory**: `specs/vk/dcf7-vk-mcp-add-tool/`  
**Task**: `dcf7-vk-mcp-add-tool`

## Problem

The MCP Servers settings view can verify saved server assignments, but after a
successful check it only shows small status icons. Users cannot see how many
tools the MCP server exposed or when the displayed result was obtained. This
makes a healthy-looking result hard to evaluate and quickly become ambiguous.

The supplied Ohana reference establishes the desired information hierarchy: a
compact server-card metadata line containing the tool count and checked time.

## User stories

- As a user managing MCP integrations, I want to see the number of tools
  discovered by the latest successful test so I can verify the server exposes
  the expected capability set.
- As a user revisiting the settings view, I want to see when the visible result
  was checked so I can judge whether it is current.
- As a user assigning one logical server to several agents, I want differing
  observed tool counts represented honestly rather than hidden behind an
  arbitrary assignment result.

## Functional requirements

- **FR-1**: A server card MUST show tool-count metadata after at least one
  successful returned assignment result includes a tool count.
- **FR-2**: One known count MUST render with correct singular/plural wording.
- **FR-3**: Multiple identical successful counts for the same logical server
  MUST render once.
- **FR-4**: Multiple differing successful counts MUST render as the inclusive
  minimum-to-maximum range.
- **FR-5**: Failed, authentication-required, unsupported, or successful results
  without `tool_count` MUST NOT be treated as zero and MUST NOT influence the
  displayed range.
- **FR-6**: A card MUST show a last-checked time once any test response for that
  logical server is received, even when no tool count is available.
- **FR-7**: Last checked MUST represent the client-observed completion time of
  the latest response batch that included that server.
- **FR-8**: A targeted server retest MUST update only returned results and the
  timestamp for that server; unrelated server metadata MUST remain unchanged.
- **FR-9**: A full test MUST update each logical server returned by the response
  using one consistent batch completion time.
- **FR-10**: Loading/reloading the shared configuration and saving followed by a
  reload MUST clear all prior result/timestamp metadata.
- **FR-11**: Existing test status, failure diagnostic, OAuth connection,
  assignment, save/discard, and JSON editing behavior MUST remain unchanged.
- **FR-12**: The metadata MUST use the settings translation namespace and format
  the checked time with the active user locale.
- **FR-13**: The metadata MUST remain readable and wrapping-safe on narrow cards.

## Acceptance scenarios

1. Given a server test returns `ok` with `tool_count: 11`, the card displays
   “11 tools” and a localized checked time.
2. Given one successful assignment reports `tool_count: 1`, the card displays
   singular “1 tool”.
3. Given two successful assignments both report 36, the card displays “36
   tools”, not duplicate values.
4. Given successful assignments report 34 and 36, the card displays the range
   “34–36 tools”.
5. Given all returned results fail or omit `tool_count`, the card displays the
   checked time but no false tool count.
6. Given server A and B already have metadata, targeting a retest of A replaces
   A's metadata while B's remains unchanged.
7. Given saved configuration is reloaded, no stale tool count or checked time is
   displayed until another test completes.

## Non-functional requirements

- No new runtime dependency.
- No backend request, response, database, or generated-type change.
- Pure aggregation/formatting behavior has focused automated tests.
- Repository formatting and frontend validation pass.

## Out of scope

- Persisting check metadata across settings sessions or page reloads.
- Automatic or scheduled health checks.
- Per-assignment count labels on the card.
- Changing how MCP probes discover tools or classify failures.

## Assumptions

- The client clock is adequate for the ephemeral checked time.
- A logical server's assignment probes in one API response constitute one
  completed check batch for display purposes.
- A range is the most compact truthful representation when executor-native
  behavior yields different visible tool sets.
