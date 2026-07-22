# Contracts: MCP Check Summary

## Pure aggregation contract

Given assignment test results for one logical server:

- Ignore results whose status is not `ok`.
- Ignore `null`/missing tool counts.
- Return `null` when no known successful count remains.
- Otherwise return `{ minimum, maximum }` across known successful counts.
- Equal values are presented as a single localized count.
- Unequal values are presented as a localized inclusive range.

## Timestamp update contract

After a shared MCP test request resolves:

1. Capture one completion timestamp.
2. Collect unique `server_name` values from the returned result list.
3. Replace result entries using the existing assignment-key merge.
4. Set the captured timestamp for each returned logical server only.
5. Do not alter timestamps for logical servers absent from the response.

## Invalidation contract

Whenever shared configuration load/save refresh clears `testResults`, it also
clears `checkedAtByServer`. Retesting does not clear old metadata before the
replacement response arrives.

## Presentation contract

- Known equal count: localized singular/plural count text.
- Known range: localized `minimum–maximum tools` text.
- Known checked time: localized “checked {{time}}” text, using a locale-formatted
  date/time value.
- When both exist, join them visually with a middle-dot separator.
- When only checked time exists, render it alone.
- When neither exists, render no metadata line.

## Network contract

None. Existing shared MCP endpoints and generated DTOs are unchanged.
