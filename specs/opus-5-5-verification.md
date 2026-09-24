# Claude Code 2.1.281 / Opus 5.5 verification

Task: vk/129c-update-to-opus-5. User explicitly requested upgrading the CLI and
enabling Opus 5.5; this authorizes the manual bump of the normally Renovate-managed
pin. The task's ordered SpecKit artifacts reside in the homelab companion repo
under `specs/vk/129c-update-to-opus-5/`.

## Upstream contract

The npm registry and Anthropic CHANGELOG publish 2.1.281. Anthropic's
[model configuration](https://code.claude.com/docs/en/model-config) requires
>=2.1.280 for Opus 5.5 and documents `opus` resolving to `claude-opus-5-5` on
Anthropic. Explicit model selection uses that same identifier. The release notes
were reviewed, including session-resumption fixes. Existing Opus 5 stays selectable.

## Artifact evidence

Downloaded the official npm `@anthropic-ai/claude-code-linux-x64@2.1.281`
tarball and verified its SHA-1 against npm registry metadata. Its native binary
SHA-256 is `56fe3da88458465fb27d7e9299dddb3fead55750fb9c2de795f233b5eea6dce1`.
Inspected the embedded JavaScript (not SDK schema titles):

- Timeout constants remain 120000/600000. `Awe` reads `BASH_DEFAULT_TIMEOUT_MS`,
  accepts positive parsed values, otherwise returns 120000. `Cwe` reads
  `BASH_MAX_TIMEOUT_MS`, clamps to `Math.max(requested, Awe(env))`, otherwise
  returns `Math.max(600000, Awe(env))`.
- Canonical background tool names Monitor, TaskStop, CronCreate, CronDelete,
  CronList and ScheduleWakeup remain present in the artifact. The alias map
  explicitly maps KillShell/KillBash to TaskStop.
- TaskOutput, AgentOutputTool, BashOutputTool, AgentOutput and BashOutput remain
  in the legacy tool-name set; retain these defensive deny entries. Their presence
  is not evidence that deprecated tools are enabled.
- `claude-opus-5-5` is present in the embedded model catalog.

No new background control is introduced. Existing parameter-level hooks and
background denial tests remain the enforcement boundary; artifact inspection
alone does not establish live account entitlement or a new end-to-end denial test.

## Acceptance

The executor catalog exposes Opus 5.5 with effort choices; explicit selection
reports 1M context; existing Opus 5 and the `opus` default remain. The CLI launch
pin and its artifact-verification guard both use 2.1.281. Validate with the Claude
executor Rust tests and independent Codex review.

Validation completed: frozen pnpm install and repository format passed;
`SQLX_OFFLINE=true cargo test -p executors executors::claude --lib` passed all
75 tests. Independent `codex review --uncommitted` reported no actionable defects.
