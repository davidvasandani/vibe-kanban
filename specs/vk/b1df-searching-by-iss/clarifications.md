# Clarifications: Search by Issue ID

## 1. Which issue fields are searchable?

**Decision:** Match only the human-readable `simple_id` in this task.

The request is specifically for searching by Issue ID, and title/description
search materially expands relevance, indexing, and result-expectation concerns.
Those fields remain explicitly out of scope.

## 2. How should issue results be labeled?

**Decision:** Lead with the issue title and place `simple_id` at the start of the
secondary context, followed by organization/project context.

This follows the existing result shape, where `title` is the primary label and
`context` disambiguates it, while keeping the searched identifier plainly
visible. The internal UUID remains available only for identity/navigation.

## Remaining questions

None.
