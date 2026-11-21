# Research: Upgrade Codex Models

## Decision: Use the pinned app-server catalog

The official OpenAI latest-model resolver currently points to GPT-6.2 Astra and
Sol for API applications. The deployed Vibe Kanban runtime, however, launches
Codex CLI 0.155.1 and sends model values to its app-server. An initialized
`model/list` request with hidden entries included returned:

| Visible | ID | Supported effort |
| --- | --- | --- |
| no | `gpt-reserve` | low through max |
| yes | `gpt-6-astra` | low through ultra |
| yes | `gpt-6-sol` | low through ultra |
| yes | `gpt-6-luna` | low through max |
| yes | `gpt-5.6-sol` | low through ultra |
| yes | `gpt-5.6-terra` | low through ultra |
| yes | `gpt-5.6-luna` | low through max |
| no | `codex-auto-review` | low through max |

Decision: mirror the six visible entries exactly. This follows project knowledge
and constitution principle IX, which prefer evidence from the pinned executable
for vendor-facing capability contracts.

## Decision: Replace rather than append

The prior exact catalog still advertises GPT-5.5 and GPT-5.3 Codex Spark. They
are absent from the deployed visible menu. Project knowledge explicitly treats
an exact current picker as a replacement contract.

Decision: remove those two advertised entries. Do not migrate stored profiles;
the picker keeps explicit current values representable and launches pass model
strings through.

## Decision: Preserve Vibe Kanban label style

App-server labels are hyphenated (`GPT-6-Astra`), while Vibe Kanban's existing
user-facing style is spaced (`GPT-6 Astra`). The identifier, not punctuation in
the label, is the vendor contract.

Decision: use `GPT-6 Astra`, `GPT-6 Sol`, and `GPT-6 Luna` for consistency with
the existing UI and retain the corresponding GPT-5.6 labels.

## Alternatives Rejected

- **Add GPT-6.2 Astra/Sol now:** rejected because the deployed ChatGPT Codex
  catalog does not expose those API identifiers.
- **Keep every old choice for compatibility:** rejected because saved values do
  not depend on continued discovery advertisement, and doing so would leave the
  menu knowingly stale.
- **Upgrade Codex CLI in the same change:** rejected because the pin is managed
  separately and a CLI/protocol upgrade has a larger blast radius.
- **Fetch model/list dynamically in product code:** rejected as unnecessary new
  runtime plumbing for a project that intentionally maintains a fallback static
  catalog; this task refreshes that contract with verified data.

## Dependencies

No new dependency.
