# Implementation plan — vk/5c0e-vk-browser

1. Apply the workspace SpecKit commands using their task directory pointer (`homelab/specs/vk/5c0e-vk-browser`); keep application code in vibe-kanban. Record existing constitution constraints and acceptance criteria.
2. Extend guarded browser actions with a fixed-purpose selected-text read; reuse the action result value and existing Type action for plain-text paste. Do not enable arbitrary evaluation.
3. Add a local editable keyboard bridge to BrowserPanelContainer for touch input and an explicit keyboard button. Focus synchronously in the user gesture. Handle composition, deletion, Enter and desktop shortcuts without duplicate text.
4. Add Copy and Paste controls with a manual selectable-text fallback, visible errors, and session/control-scoped ephemeral state. Keep toolbar usable on phones and locale keys consistent.
5. Add focused input and driver/control tests; install dependencies, generate shared types, format, run type/lint and relevant tests.
6. Run independent Codex CLI review, fix findings and rerun affected checks. Record reusable knowledge and commit it. Open and merge PR(s) for application and pipeline artifacts after successful checks.

Prior knowledge: ../PRIOR_KNOWLEDGE.md. No other service changes are planned.
