# Research Notes: Search by Issue ID

## Existing search authority

Remote global search already establishes an `accessible` organization CTE using
`organization_member_metadata` and derives project/workspace results from it.
Issues are remote PostgreSQL entities attached to projects, so extending this
same query preserves the correct authorization and avoids host fan-out.

## Identity and navigation

`issues.simple_id` is the visible identifier users know. `issues.id` is the UUID
used by existing issue-detail routes. `searchResultHref` already emits the right
route whenever a result carries `project_id` plus `issue_id`; adding another
route implementation would duplicate a tested convention.

## Matching decision

Use `strpos(lower(simple_id), lower($2)) > 0`, matching existing metadata search.
This is parameterized, literal, case-insensitive matching and avoids SQL wildcard
semantics. Searching titles or descriptions is deferred because the request is
specifically about Issue ID and broader relevance is a separate product choice.

## Result presentation

Keep the issue title as the primary result title and begin context with
`simple_id`, followed by organization/project names. This makes the searched
value visible while fitting the existing result component without new UI fields.

## Alternatives rejected

- Search issues from currently loaded Electric collections: incomplete across
  organizations and dependent on active shapes.
- Search issue UUIDs: internal identity is not the user-visible Issue ID.
- Return matching workspaces as a proxy for issues: not every issue has a
  workspace and it obscures the direct issue destination.
- Add a separate issue-search endpoint: duplicates authorization, deadline, and
  aggregation behavior already supplied by global search.

## Dependencies

No new dependency is needed.
