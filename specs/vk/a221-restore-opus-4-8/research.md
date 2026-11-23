# Research: Restore Opus 4.8

## Decision: ID, label, capabilities
- **Source**: `@anthropic-ai/claude-code@2.1.322` native artifact
  (`bin/claude.exe`, sha256
  `bfb3703ad5380242328459bc62ada4bde8ae7ba57c6e1eeadb9c00e08874c478`),
  embedded model catalog record:
  `{id:"claude-opus-4-8", family:"opus", display_name:"Opus 4.8",
  context:{window:1e6, native_1m:!0, supports_1m_beta:!0, supports_1m_suffix:!0},
  max_output_tokens:{default:64000, upper:128000},
  capabilities:["effort","max_effort","xhigh_effort","adaptive_thinking",...],
  default_effort:"high"}`.
- **Cross-check**: Anthropic's model reference lists `claude-opus-4-8` as Active
  with a 1M context window.
- **Decision**: label "Opus 4.8"; efforts low/medium/high/xhigh/max; 1M window.

## Why Opus 4.8 was missing
`#329` (ec267899) replaced alias entries with an explicit versioned list
(Opus 5.5, Opus 5, Sonnet 5, Fable 5.1, Haiku 4.5), and `#397` refreshed it for
2.1.322. Opus 4.8 was never in the Claude Code catalog's versioned form. The
`context_window_for_model` test even asserted 200K for it, which contradicts
the CLI catalog.

## Related: CLI refusal fallback
In 2.1.281 and later, when Opus 5.5 refuses a `cyber`-category request, the CLI
retries on `claude-opus-4-8` (hard-coded map). An account that cannot reach it
sees "There's an issue with the selected model (claude-opus-4-8)". This explains
the original report and is outside this change.

## Alternatives considered
- Add `opus[1m]`-style "Opus 4.8 (1M)" entry: rejected. The CLI marks it
  `native_1m`, so the plain ID already has 1M.
- Make Opus 4.8 an alias target: rejected. The CLI's `opus` alias is Opus 5.5.
