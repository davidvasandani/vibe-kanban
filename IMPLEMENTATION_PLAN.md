# Implementation Plan: Add Atlassian CLI to Managed Tools

1. Confirm repository constraints, existing CLI catalog conventions, current
   ACLI release metadata, stable artifact URLs, archive layout, and checksums.
2. Add the ACLI identifier, pinned version, Linux amd64/arm64 sources, install
   strategy, version probe, metadata, and official documentation URL to the
   managed CLI catalog.
3. Extend focused service tests to cover ACLI catalog completeness and exact
   source/installation metadata without weakening existing invariants.
4. Regenerate shared TypeScript types so API consumers recognize `acli`.
5. Run formatting, targeted Rust tests/checks, and generated-type validation;
   correct any failures attributable to the change.
6. Run an independent Codex review of the diff and address confirmed findings
   until no significant findings remain.
7. Document the reusable managed-CLI catalog extension process in the project
   knowledge base, tag it with task `fc47-atlassian-cli-to`, refresh the index,
   and commit the knowledge-base update separately as required.
