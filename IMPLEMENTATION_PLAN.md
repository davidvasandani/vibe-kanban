# Implementation plan: Global Search

1. Follow the workspace-provisioned SpecKit commands; their exact artifact root
   is `../homelab/specs/vk/8f5e-global-search`. These are Vibe Kanban task documents,
   not changes to another service. Carry forward `../PRIOR_KNOWLEDGE.md`.
2. Add bounded read-only local search for workspace metadata and stored chat
   turn prompts/final assistant messages, without reconstructing execution logs.
3. Add membership-scoped remote search for organizations, projects and workspaces.
4. Add a shared global search dialog and API aggregation, using authenticated
   remote and local/relay transports. Show unavailable sources explicitly.
5. Integrate a search button into both shells with a keyboard shortcut. Preserve
   org/project/host/session context during result navigation.
6. Test contracts and rendered behavior; install dependencies, format, run checks
   and lint, and obtain independent Codex diff review; address findings.
7. Record reusable knowledge and validation, commit task documents and code, open
   and merge PRs against the actual repository base branches.
