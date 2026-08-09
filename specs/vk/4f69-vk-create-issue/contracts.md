# Contracts: Scrollable Create-Issue Settings

## External Contracts

No HTTP API, database, generated TypeScript, persistence, translation, or
deployment contract changes.

## Rendered Layout Contract

`KanbanIssuePanel` must render:

- one full-height column shell that clips its own overflow;
- one non-shrinking header outside the body;
- one body that fills remaining space, has zero minimum height, and owns
  vertical auto overflow;
- all create-mode settings and create actions inside that body.

In utility-class terms, the body contract is equivalent to:

```text
min-h-0 flex-1 overflow-y-auto
```

The exact class order is not contractual.

## Behavior Preserved

- Header visibility and close behavior
- Create/edit form data and callbacks
- Existing property, tag, title, description, pipeline, attachment, and section
  order
- Create button enabling/submission and draft deletion
- Edit-mode scrolling and trailing sections
- Shared local-web and remote-web component usage

## Test Contract

A rendered component test must verify shell/body layout classes and confirm the
draft-workspace control and Create Issue action are descendants of the body in
create mode.
