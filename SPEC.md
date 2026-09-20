# Issue references with explicit destinations

Make issue references in Vibe Kanban agent conversations navigable without requiring agents or users to infer URLs. Provide authoritative issue URLs in issue-facing MCP results and instruct agents to use those URLs when referring to issues. Audit the existing rendering and route contracts before choosing any additional presentation changes. Existing explicit Markdown links must continue to work.

Acceptance: issue creation, retrieval, lists, and related-issue results expose deterministic destinations derived from authoritative project/issue identities as application-relative browser routes; issue references use Markdown links; missing configuration must not produce fabricated hosts. Verify route compatibility, escaping and all affected output shapes. Changes are confined to the Vibe Kanban service repository. No unrelated service deployment changes.

Validation: focused regression tests for URL construction and issue response coverage, repository formatting and applicable checks, independent Codex review, knowledge-base update, pull request and merge.

Read-only Lexical rendering currently disables relative links. Permit only the exact project/issue UUID route; retain existing handling for all other links. MCP results and instructions address newly generated prose; existing bare historical keys are not rewritten.
