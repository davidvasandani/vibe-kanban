# Keep 1Password references readable

Task: vk/a63c-don-t-obfuscate.

The organization environment-variable card currently uses password inputs for all
values, hiding 1Password reference paths as shown in the attached screenshots.
For both add and replacement-value inputs, display values beginning with the
exact `op://` prefix as text. Keep empty values and literal secrets masked.
Reevaluate visibility on every edit, including pasting and replacing a reference
with a literal. Preserve the input value exactly and keep existing mutation,
encryption, resolution, and saved-value redaction behavior unchanged.

Scope: Vibe Kanban shared frontend only. No deployment or other service changes.
Validation: exercise prefix transitions in rendered inputs and verify frontend
checks, formatting, and independent diff review before opening and merging a PR.
