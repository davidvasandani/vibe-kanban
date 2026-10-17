# `/speckit.clarify`: Claude Fable 5.1 and Claude Code Refresh

## Resolved questions

1. **What model ID should Vibe Kanban pass for Fable 5.1?**

   Use `claude-fable-5-1`. Anthropic's model documentation names that Claude
   API ID, and the `@anthropic-ai/claude-code-linux-x64@2.1.268` executing
   artifact includes it in its baked-in model catalog with display name
   `Fable 5.1` and family `fable`.

2. **What does “latest Claude Code” mean for this task?**

   Pin `@anthropic-ai/claude-code@2.1.268`. On 2026-09-11 the npm registry's
   `latest` tag and the platform-native package version both resolve to
   `2.1.268`. Version `2.1.269` exists only on the `next` prerelease channel;
   the task does not opt into prerelease dependencies. The older `stable`
   channel points to 2.1.236, so `latest` is the correct interpretation of the
   user's request.

3. **Which effort choices does Fable 5.1 support?**

   The 2.1.268 native catalog gives Fable 5.1 the `effort`, `max_effort`, and
   `xhigh_effort` capabilities with default `high`. Vibe Kanban should retain
   its established five-choice set: `low`, `medium`, `high`, `xhigh`, `max`.

4. **Did the reviewed safety identifiers change?**

   No. The 2.1.268 Linux x64 binary contains the canonical names `Monitor`,
   `TaskOutput`, `TaskStop`, `CronCreate`, `CronDelete`, `CronList`, and
   `ScheduleWakeup`. Its embedded alias table still maps `BashOutput`,
   `BashOutputTool`, `AgentOutput`, and `AgentOutputTool` to `TaskOutput`, and
   `KillShell` and `KillBash` to `TaskStop`. The existing deny list remains
   correct. The parameter-level `Bash` guard remains mandatory because the
   binary still includes `run_in_background` and `Read` remains part of the
   actual output-reading path.

5. **Did the model aliases require a catalog removal or default change?**

   No. The 2.1.268 native binary recognizes the aliases `sonnet`, `opus`,
   `haiku`, and `fable`, plus their applicable context variants. Its catalog
   defines current first-party families as Fable 5.1, Opus 5, Sonnet 5, and
   Haiku 4.5. Vibe Kanban keeps its existing default `opus` and existing
   entries while adding the explicit Fable 5.1 ID.

## Evidence

- Anthropic model overview and “What's new” pages for Claude Fable 5.1.
- npm registry metadata for `@anthropic-ai/claude-code` and
  `@anthropic-ai/claude-code-linux-x64`, queried 2026-09-11.
- The extracted ELF binary from
  `@anthropic-ai/claude-code-linux-x64@2.1.268`, including its baked-in model
  catalog, model aliases, capability flags, tool registry, and alias map.
- Anthropic's Claude Code 2.1.268 changelog, including Fable-specific fixes and
  no announced removal of Vibe Kanban's existing model aliases.
