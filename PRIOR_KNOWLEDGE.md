# Prior knowledge: Global Search

Searched the existing `vibe-kanban/wiki` and `vibe-kanban/docs/knowledge-base`
for search, chat, organization, navigation and authorization. The knowledge base
is populated; no pages were modified during recall.

- `wiki/appbar-rail-and-org-tiles.md`: organization selection lives in
  useOrganizationStore; useUserOrganizations obtains accessible organizations.
  RemoteAppShell scopes projects by active organization. Global search must
  deliberately query beyond that active organization.
- `wiki/electric-sync-fallback.md`: Electric collections are cached and can fall
  back to REST. Avoid tying search completeness to currently mounted shapes.
- `wiki/workspace-navbar-breadcrumbs.md`: workspace issue_id is a UUID, while
  simple_id is display identity. Navigate using authoritative IDs and preserve
  project/workspace context when asynchronous data is unavailable.
- `docs/knowledge-base/lazy-loading-normalized-conversation-history.md`:
  conversation history is paged; loading displayed chat is not a complete global
  search source. Search persisted content on the server with bounded results.

Task: vk/8f5e-global-search
