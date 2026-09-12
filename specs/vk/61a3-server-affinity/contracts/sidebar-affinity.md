# Internal Contract: Sidebar Affinity Summary

## Input

The selected workspace summary may expose:

- `serverAffinity.worker_hostname`
- `serverAffinity.requested_worker_hostname`
- `serverAffinity.kind`

The summary itself or `serverAffinity` may be absent during loading/selection
changes.

## Header output

1. Render `worker_hostname` when present.
2. Otherwise render `requested_worker_hostname` when present.
3. Otherwise render the translated label for `kind`.
4. Render no header metadata when affinity summary data is absent.
5. Keep output to one truncatable line and preserve the disclosure caret.

## Expanded body output

- “Current server” and its value occupy one compact aligned row.
- Non-local placements expose “Run on” and the existing placement selector in a
  second aligned row.
- Local placements retain the existing explanatory text instead of the select.

## Invariants

- This rendering contract performs no affinity mutation or query.
- Placement semantics, eligibility, confirmation, restart, and error behavior
  remain owned by `ServerAffinitySectionContainer` and existing APIs.
