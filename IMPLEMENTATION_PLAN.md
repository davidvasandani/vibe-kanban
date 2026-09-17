# Implementation plan — Atlassian Rovo reconnect

1. Trace configured gateway identity through OAuth start, callback, and manual completion; compare reconnect matching with identifier migrations and URL adapters.
2. Complete SpecKit constitution, specification, clarification, plan, tasks, and analysis in order.
3. Preserve the original connection identity across reconnect and update only assignments still matching the original configured server; retain unrelated entries.
4. Add focused regression tests for reconnect after identifier migration and ordinary connect/reconnect, including changed/deleted definitions.
5. Install frozen dependencies, format, and run targeted checks. Review the diff independently with Codex CLI and address confirmed findings.
6. Update and commit reusable knowledge, open a pull request against the verified base branch, and merge after required checks.
