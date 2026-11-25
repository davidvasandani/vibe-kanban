# Research: Align Codex Model Catalog

## Method

- Downloaded `@openai/codex@0.171.0` (npm `latest` on 2026-11-23) into an
  isolated directory. Ran `codex app-server`, then `initialize` and
  `model/list {includeHidden: true}`, with a scoped `CODEX_HOME` whose
  `auth.json` is a symlink to the shared credential (the worker pattern), so
  the shared models cache and token rotation were untouched.
- Drove the 0.171.0 TUI `/model` picker in a pty (no turn submitted).
- Sent one-word `codex exec` smoke prompts per new model on 0.155.1 (deployed)
  and 0.171.0.

## Findings

| ID | Hidden | Efforts | 0.155.1 lists | 0.155.1 runs | 0.171.0 runs |
| --- | --- | --- | --- | --- | --- |
| gpt-6.2-sol | no | low–ultra | no | **no** | yes* |
| gpt-6.2-astra | no | low–ultra | no | **no** | yes |
| gpt-6.1-terra | no | low–ultra | no | **no** | yes |
| gpt-6.1-sol | no | low–ultra | no | **no** | yes* |
| gpt-6-astra / sol | no | low–ultra | yes | yes | — |
| gpt-6-luna | no | low–max | yes | — | — |
| gpt-5.6-sol / terra | no | low–ultra | yes | — | — |
| gpt-5.6-luna | no | low–max | yes | — | — |
| gpt-reserve | **yes** | low–max | hidden | — | — |
| codex-auto-review | **yes** | low–max | hidden | — | — |

\* listed as visible by 0.171.0. Smoke-tested: 6.2 Astra and 6.1 Terra.
"—" = not exercised (unchanged entries).

The 0.155.1 error was `400 model_not_found`, subreason
`model_not_in_accessible_catalog`. The backend filters the catalog and the
allowed models by `client_version`.

## Decisions

- **Membership and efforts:** take them from the 0.171.0 `model/list`
  visible records. Rejected: 0.155.1 `model/list` (stale; it was PR #404's
  wrong assumption).
- **Order:** use the supplied selector order (see `clarifications.md` Q1).
  Rejected: TUI backend-priority order, because the task says otherwise.
- **Labels:** VK spaced style.
- **No CLI bump here:** the npx pin is Renovate-managed, and the effective
  runtime is homelab `pkgs.codex`, which is out of scope. Its upgrade is the
  ship gate.
- **No dynamic fetch:** that would be new runtime plumbing and would mirror
  the deployed 0.155.1 list anyway, which is exactly the stale list.

## Dependencies

None added.
