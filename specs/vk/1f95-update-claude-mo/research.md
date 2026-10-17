# Research: Claude Fable 5.1 and Claude Code 2.1.268

## Decision: use `claude-fable-5-1`

Anthropic's Fable 5.1 documentation names `claude-fable-5-1` as its Claude API
ID. The exact same identifier and `Fable 5.1` display name appear in the baked-in
model catalog extracted from the Claude Code 2.1.268 Linux x64 binary. This is
stronger than inferring a name from previous releases.

## Decision: pin npm `latest`, not `next` or `stable`

Registry state on 2026-09-11:

- `latest`: 2.1.268
- `stable`: 2.1.236
- `next`: 2.1.269

The user asked for the latest Claude Code dependencies. Version 2.1.268 is the
current generally published `latest`; choosing `next` would opt into a separate
prerelease channel, while choosing `stable` would deliberately lag the request.
The wrapper's optional dependencies pin each supported native platform package
to exactly 2.1.268.

## Decision: retain five effort levels

The native 2.1.268 model catalog marks Fable 5.1 with `effort`, `max_effort`,
and `xhigh_effort`, plus adaptive thinking and a default effort of `high`.
Therefore the existing Vibe Kanban set `low`, `medium`, `high`, `xhigh`, `max`
is correct for both the alias `fable` and explicit Fable 5.1 entry.

## Decision: classify every current native-1M catalog choice consistently

The native catalog declares `context.window: 1_000_000` and `native_1m: true`
for `claude-fable-5-1`; it also declares native 1M context for Opus 5 and
Sonnet 5. Vibe Kanban seeds context utilization from the selected or reported
model before the final usage event arrives. Therefore `opus`, `sonnet`, and
`fable`, plus their explicit current catalog IDs, must resolve to
`CLAUDE_1M_CONTEXT_WINDOW` rather than the 200K fallback. Haiku remains on the
fallback unless its selected identifier has the explicit `[1m]` suffix.

## Decision: preserve existing safety names

The executing binary retains:

- Canonical tools: `Monitor`, `TaskOutput`, `TaskStop`, `CronCreate`,
  `CronDelete`, `CronList`, `ScheduleWakeup`.
- Task-output aliases: `BashOutput`, `BashOutputTool`, `AgentOutput`,
  `AgentOutputTool` -> `TaskOutput`.
- Task-stop aliases: `KillShell`, `KillBash` -> `TaskStop`.

It also retains `Bash`, `Read`, and `run_in_background`; consequently, the
existing `PreToolUse` parameter check on background Bash remains the real
enforcement point. No safety identifier should be guessed or removed.

## Decision: preserve current aliases and default

The binary recognizes the base aliases `sonnet`, `opus`, `haiku`, and `fable`.
Its catalog names Fable 5.1, Opus 5, Sonnet 5, and Haiku 4.5 as current family
models. The release notes do not announce removal of the existing aliases.
Vibe Kanban's existing `opus` default remains a product choice and is unchanged.

## Claude Code release review

The 2.1.268 changelog is a broad patch release. Relevant items include fixes
for Fable long-context messaging, model-access cache consistency, print-mode
permission hooks, policy-helper warnings in headless mode, and permission-rule
handling. It does not announce a breaking noninteractive JSON protocol change
or a model-alias removal. The complete change remains subject to focused tests
and independent diff review.

## Deployment inspection boundary

`homelab/modules/vibe-kanban-rebuild.nix` is the only in-scope deployment file.
It installs a host `claude-code` package among worker prerequisites, while the
executor source launches its own npm-pinned wrapper. The module should change
only if runtime inspection finds that its Node/package setup cannot satisfy the
new wrapper's Node >=22 requirement or overrides the executor command.

## Sources

- https://platform.claude.com/docs/en/models/fable-5-1/overview
- https://platform.claude.com/docs/en/models/fable-5-1/whats-new-fable-5-1
- https://github.com/anthropics/claude-code/blob/main/CHANGELOG.md
- npm registry metadata and extracted
  `@anthropic-ai/claude-code-linux-x64@2.1.268` artifact, queried 2026-09-11
