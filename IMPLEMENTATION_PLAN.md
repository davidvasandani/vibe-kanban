# Implementation plan — durable wake-ups

1. Complete SpecKit constitution/spec/clarification/design/tasks/analysis stages
   using the workspace command feature pointer. Preserve service scope.
2. Inspect durable continuation delivery and settled-session admission. Define a
   persisted wake-up record with deadline, target, prompt, state and trigger data.
3. Add scheduling validation, cancellation and restart reconciliation; enforce
   seven-day, eight-pending and bounded automatic-continuation limits atomically.
4. Integrate timer and successful interval-completion delivery with existing
   follow-up execution, preserving queued operator work and warm session gates.
5. Expose schedule/list/cancel through MCP/API and show countdown, deadline,
   prompt and cancellation in the existing drawer.
6. Generate shared types, add lifecycle/API/UI regression coverage, install
   dependencies, format and run relevant checks.
7. Run independent Codex review, fix confirmed findings and reverify. Record and
   commit reusable project knowledge. Open and merge the task PR.
