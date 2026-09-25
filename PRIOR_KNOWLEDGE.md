# Prior knowledge — vk/a63c-don-t-obfuscate

Searched the populated `vibe-kanban/wiki` and `vibe-kanban/docs/knowledge-base`
for `op://`, `1Password`, environment variables, and secret handling (read-only).

- `docs/knowledge-base/workspace-environment-inheritance.md`: organization values
  are encrypted strings. Only whole values starting with exact `op://` are
  references. No trimming, interpolation, or recursive resolution. Resolution is
  at process preparation and resolved values must never return to settings.
- `wiki/external-connector-sync.md`: saved secret reads expose metadata, not
  plaintext. Keep this task confined to the locally entered draft.
- Source corroboration: `OrganizationEnvVarsCard.tsx` has password inputs in
  both add and edit flows, and redacts saved values independently.

Implication: derive draft input visibility using the same exact prefix rule;
leave storage, resolution, API responses, and saved-row masking intact.
