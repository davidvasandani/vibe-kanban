# Codex Model Discovery Contract

`StandardCodingAgentExecutor::discover_options` for Codex returns a model
selector whose explicit entries are exactly:

| Order | ID | Label | Reasoning options |
| ---: | --- | --- | --- |
| 1 | `gpt-6-astra` | GPT-6 Astra | low, medium, high, xhigh, max, ultra |
| 2 | `gpt-6-sol` | GPT-6 Sol | low, medium, high, xhigh, max, ultra |
| 3 | `gpt-6-luna` | GPT-6 Luna | low, medium, high, xhigh, max |
| 4 | `gpt-5.6-sol` | GPT-5.6 Sol | low, medium, high, xhigh, max, ultra |
| 5 | `gpt-5.6-terra` | GPT-5.6 Terra | low, medium, high, xhigh, max, ultra |
| 6 | `gpt-5.6-luna` | GPT-5.6 Luna | low, medium, high, xhigh, max |

The separate Default choice is model-picker infrastructure and is not an entry
in this table. Hidden app-server models and superseded public choices are not
advertised. Permissions, aliases, slash commands, and stored profile values are
unchanged.
