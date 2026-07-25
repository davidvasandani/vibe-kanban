# Implementation Plan: Grok Executor Auto Mode

1. Confirm the Grok-specific permission-to-ACP-mode contract against the
   existing executor and shared ACP harness.
2. Refresh the SpecKit constitution and generate the task's canonical feature
   artifacts.
3. Clarify any remaining ambiguity in the generated feature specification,
   especially the expected `Auto` and `Supervised` ACP mode identifiers.
4. Generate the SpecKit technical plan, research notes, data-model/contracts
   only where applicable, and dependency-ordered task list.
5. Run SpecKit analysis and resolve any gaps or constitution conflicts before
   changing production code.
6. Add a Grok-local helper that maps the existing `yolo` permission state to
   ACP `auto` or `ask`.
7. Configure every Grok `AcpAgentHarness` with that mapped mode, covering both
   initial and follow-up execution through their shared harness constructor.
8. Add focused unit tests for Auto and Supervised mode mapping while retaining
   command-order and serialization coverage.
9. Run formatting and focused executor tests, then the relevant broader checks
   required by repository guidance.
10. Run an independent Codex diff review, address confirmed findings, and
    repeat verification until no significant findings remain.
11. Update the Grok executor knowledge page and knowledge-base index metadata
    with the reusable ACP mode lesson, tagged with task `ba9f-grok-vk-executor`,
    then commit the knowledge-base update.
