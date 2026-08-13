# Clarifications: Single-Value Browser Titles

## Q1. How should whitespace-only labels be handled?

**Decision**: Treat a whitespace-only label as absent and continue to the next
fallback. Trim surrounding whitespace from the meaningful candidate selected
for browser metadata.

**Basis**: A whitespace-only browser title is visually blank and defeats the
required stable fallback. Trimming browser metadata is deterministic, does not
rewrite persisted user text, and matches the normalization browsers apply when
exposing `document.title`.

## Q2. Does “especially the ticket number” remove IDs from breadcrumbs?

**Decision**: No. Ticket IDs must not be concatenated into browser-tab titles,
but visible workspace breadcrumbs retain them.

**Basis**: The task targets the concatenated browser title shown in the
screenshot. Constitution principle VII separately requires workspace
breadcrumbs to preserve issue identity, and the task does not ask to weaken
visible navigation.

## Remaining Questions

None.
