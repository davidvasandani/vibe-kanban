# `/speckit.clarify`: Upgrade Codex Models

## Resolved Questions

1. **Which source defines “current” for this picker?** The deployed ChatGPT
   Codex app-server, not the public API model guide. Vibe Kanban launches the
   pinned CLI and passes the selected ID to it, so its `model/list` response is
   the compatibility boundary.
2. **What exact catalog is in scope?** Codex 0.155.1 reports six non-hidden
   models in order: `gpt-6-astra`, `gpt-6-sol`, `gpt-6-luna`, `gpt-5.6-sol`,
   `gpt-5.6-terra`, and `gpt-5.6-luna`. Internal `gpt-reserve` and
   `codex-auto-review` entries are hidden and must not appear.
3. **Should older advertised models be retained?** No. The authoritative menu
   is exact, and project knowledge says an exact refresh removes superseded
   advertised entries. GPT-5.5 and GPT-5.3 Codex Spark disappear from discovery.
4. **Will removal break saved profiles?** No migration is required. Picker
   infrastructure keeps an explicit current value representable, and Codex
   execution passes stored model IDs through. Removal affects discovery, not
   stored configuration.
5. **Which reasoning levels apply?** GPT-6 Astra, GPT-6 Sol, GPT-5.6 Sol, and
   GPT-5.6 Terra support through `ultra`. GPT-6 Luna and GPT-5.6 Luna support
   through `max` only.
6. **Should Vibe Kanban use app-server's hyphenated display names?** No. Display
   labels follow the established Vibe Kanban style (`GPT-6 Astra`, not
   `GPT-6-Astra`); model IDs and capabilities remain exact.
7. **Should the CLI be upgraded to obtain GPT-6.2?** No. CLI pins are managed
   separately, and GPT-6.2 is not in this deployed CLI's ChatGPT catalog.

## Remaining Open Questions

None.
