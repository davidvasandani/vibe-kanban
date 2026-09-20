# Implementation plan — issue links

1. Follow workspace SpecKit commands, which target homelab/specs/vk/f31e-issue-link; keep service implementation in vibe-kanban.
2. Add explicit application-relative issue URLs to MCP issue creation, list, detail, update, relationships, subissues and active context using authoritative project/issue UUIDs. No backend origin inference.
3. Add MCP instructions and tool descriptions requiring Markdown links for issue references using returned destinations.
4. Verify conversation Markdown preserves application-relative issue links, including the read-only click handler; fix only if necessary.
5. Add focused response/route regression tests, install frozen dependencies, run formatting and relevant checks.
6. Run independent Codex CLI review, resolve significant findings, document actual validation and reusable knowledge.
7. Commit, open pull requests against the workspace base branches, and merge after checks permit.
