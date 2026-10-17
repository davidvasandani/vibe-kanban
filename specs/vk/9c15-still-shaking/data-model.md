# Data Model: Earlier-History Control State

## Existing Inputs

- `hasEarlierHistory: boolean`
- `isLoadingEarlier: boolean`
- `loadEarlierError: string | null`
- `requestEarlierHistory(): Promise<void>`

## Presentation States

| State | Predicate | Presentation | Geometry contract |
| --- | --- | --- | --- |
| idle | history exists, not loading, no error | Load-earlier button | reserved stable block |
| loading | loading | skeleton and status | same reserved stable block |
| retry | error, not loading | Retry button and error status | stable control block; error may extend below |
| absent | no history, not loading, no error | nothing | no reserved block |

No state is persisted. The semantic history anchor remains separate and retains
its existing `semanticKey`, viewport top, and scroll-height evidence.
