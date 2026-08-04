# Component Contract

## `CollapsibleSectionHeader`

New optional prop:

```ts
fillAvailableSpace?: boolean;
```

Contract when `false` or omitted:

- Existing root sizing and all collapse behavior remain unchanged.

Contract when `true`:

- An expanded (or non-collapsible) section root grows and shrinks as an equal
  participant in its parent's remaining vertical space.
- A collapsed section root uses intrinsic header height and does not grow or
  shrink.
- Expansion state, persistence, actions, and rendered children retain their
  existing behavior.

This is an internal presentational TypeScript interface, not a network API.
