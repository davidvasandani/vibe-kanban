# Contract: Git Header Behind Status

## Inputs

- Selected workspace ID, optional during workspace loading.
- Ordered repository metadata list.
- Existing branch-status query result, which may be absent while loading.

## Output

- `null` when the workspace ID/status is absent or no repository is positively
  behind.
- One bounded header metadata element otherwise.

## Copy contract

| Workspace repositories | Positive behind values | Visible text |
| --- | --- | --- |
| one | repo = 3 | `3 behind` |
| one | repo = 0/null | no output |
| multiple | web = 2, server = 5 | `web 2 · server 5` |
| multiple | web = 0, server = 5 | `server 5` |

Accessible/title copy expands each visible value, for example:
`web is 2 commits behind; server is 1 commit behind`.

## Lifecycle contract

- The subscription is keyed by selected workspace ID.
- The output updates with the shared branch-status query.
- The subscription remains mounted independent of Git body disclosure state.
- No new HTTP request shape or polling interval is introduced.
