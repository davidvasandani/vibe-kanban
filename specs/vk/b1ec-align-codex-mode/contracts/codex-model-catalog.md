# Codex Model Discovery Contract

`Codex::discover_options` returns `model_selector.models` exactly as below.
The infrastructure Default choice is not an entry here.

| # | ID | Label | Reasoning options |
| ---: | --- | --- | --- |
| 1 | `gpt-6.2-astra` | GPT-6.2 Astra | low, medium, high, xhigh, max, ultra |
| 2 | `gpt-6.2-sol` | GPT-6.2 Sol | low, medium, high, xhigh, max, ultra |
| 3 | `gpt-6.1-sol` | GPT-6.1 Sol | low, medium, high, xhigh, max, ultra |
| 4 | `gpt-6.1-terra` | GPT-6.1 Terra | low, medium, high, xhigh, max, ultra |
| 5 | `gpt-6-astra` | GPT-6 Astra | low, medium, high, xhigh, max, ultra |
| 6 | `gpt-6-sol` | GPT-6 Sol | low, medium, high, xhigh, max, ultra |
| 7 | `gpt-6-luna` | GPT-6 Luna | low, medium, high, xhigh, max |
| 8 | `gpt-5.6-sol` | GPT-5.6 Sol | low, medium, high, xhigh, max, ultra |
| 9 | `gpt-5.6-terra` | GPT-5.6 Terra | low, medium, high, xhigh, max, ultra |
| 10 | `gpt-5.6-luna` | GPT-5.6 Luna | low, medium, high, xhigh, max |

Never advertised: `gpt-reserve`, `codex-auto-review` (hidden).

Runtime precondition: entries 1–4 need a Codex CLI newer than 0.155.1
(verified on 0.171.0).
