# Claude Code 2.1.322 / Fable 5.2 + Mythos 5.2 verification

Task: `vk/7c99-add-new-claude-m`. The user chose to add the models **and** bump
the CLI (option B). That choice is the release-note review required by the
`@anthropic-ai/claude-code` carve-out. It authorizes this hand bump of the
normally Renovate-managed pin. The SpecKit artifacts are in the homelab
companion repo under `specs/vk/7c99-add-new-claude-m/`.

## Artifact

- `@anthropic-ai/claude-code-linux-x64@2.1.322` tarball SHA-1
  `b56040e7f01b956839d56504882d36a177f06a89`. This equals npm `dist.shasum`.
- Native `claude` SHA-256
  `bfb3703ad5380242328459bc62ada4bde8ae7ba57c6e1eeadb9c00e08874c478`.
- The 2.1.281 binary was re-downloaded as a control. Its SHA-256 is
  `56fe3da8…dce1`, matching `specs/opus-5-5-verification.md`.

## Embedded model catalog and aliases (read from the binary)

```text
aliases:{opus:{default:"claude-opus-5-5",…},sonnet:{default:"claude-sonnet-5-5",…},
         haiku:{default:"claude-haiku-5-5",…},fable:{default:"claude-fable-5-2",per_provider:{gateway:"claude-fable-5"}}}
```

There is no `mythos` alias. The catalog records for `claude-fable-5-2`,
`claude-mythos-5-2`, `claude-sonnet-5-5` and `claude-haiku-5-5` all declare
`context:{window:1e6,native_1m:!0,…}` and the capabilities `effort`,
`xhigh_effort` and `max_effort`. The Mythos records have `null` third-party
provider IDs, meaning they are first-party only.

## Re-proved behaviours

| Behaviour | Evidence on 2.1.322 |
| --- | --- |
| Bash timeouts | `var a=120000,s=600000`; default `kK` and max `kX = Math.max(requested, kK)`, the same logic as 2.1.281's `Awe`/`Cwe`. |
| Background tool names | `Monitor`, `TaskOutput`, `TaskStop`, `CronCreate`, `CronDelete`, `CronList` and `ScheduleWakeup` are present. |
| Alias → canonical map | `{Task:"Agent",KillShell:"TaskStop",KillBash:"TaskStop",ListPeers:"ListAgents",Brief:"SendUserMessage",…}` is unchanged. |
| Legacy TaskOutput set | `TaskOutput`, `AgentOutputTool`, `BashOutputTool`, `AgentOutput` and `BashOutput` are unchanged. |
| Peer tools | The `ListAgents` and `ListPeers` constants and `SendMessage` are present. |
| `crossSessionInbound` | The restrictive values are `["refuse","hold"]` and the `crossSessionInbound=refuse` path is present. |
| `--resume` scope | Throwaway `CLAUDE_CONFIG_DIR` and no credentials. With empty `projects/` it prints `No conversation found`. With the transcript under another project's folder, or under `$HOME/.claude/projects`, the lookup passes and it prints `Not logged in`. |
| MCP precedence | Throwaway `HOME`, project servers approved, the same experiment run on 2.1.281 and 2.1.322 with identical results. A same-name project `vk` replaces the user `vk`. An ancestor `.mcp.json` loads from `sub/`. `--settings '{"disabledMcpjsonServers":["vk"]}'` restores the user entry and keeps `extra`. On 2.1.322, an **unapproved** project entry stays `Pending approval`, and the user entry remains active. |

## Release notes 2.1.282 → 2.1.322

The relevant items are in `homelab/specs/vk/7c99-add-new-claude-m/research.md`.
No stream-json protocol break is announced.

## Validation

Recorded on completion in the homelab `tasks.md` and in the PR bodies.
