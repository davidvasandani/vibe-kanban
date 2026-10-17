# Contract: Claude Executor Model Catalog and Launch Pin

## Discovery output

`default_discovered_options()` returns a model selector containing an entry
equivalent to:

```text
id: claude-fable-5-1
name: Fable 5.1
provider_id: none
reasoning: low, medium, high, xhigh, max
```

All pre-existing entries and `default_model = opus` remain present.

The current native-1M catalog choices return `1_000_000` from
`context_window_for_model()`: `opus`, `sonnet`, `fable`, `claude-opus-5`,
`claude-sonnet-5`, and `claude-fable-5-1`.

## Command input

When an executor profile selects `claude-fable-5-1`, the existing Claude
command builder appends:

```text
--model claude-fable-5-1
```

for initial and resumed executions without translation.

## Dependency pin

The non-router base command invokes exactly:

```text
npx -y @anthropic-ai/claude-code@2.1.268
```

No mutable npm tag is used at runtime.

## Safety invariants

- `ScheduleWakeup` and the reviewed background helper names remain disallowed.
- A `PreToolUse` hook on exact `Bash` denies only calls whose parsed input has
  `run_in_background: true` and names `spawn_poller` as the replacement.
- Foreground Bash continues to reach the ordinary permission path.
