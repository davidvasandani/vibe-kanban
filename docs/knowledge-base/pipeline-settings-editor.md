# Pipeline Settings editor

Tags: `3a97-no-frontend-for`

## Architecture boundary

Pipeline files belong to a selected machine, so Settings must access them through
the selected `MachineClient`, not through unscoped local API helpers. Query keys
include `machineClient.queryScopeKey`; successful writes, deletes, and resets
invalidate both the selected machine's status/raw queries and the legacy
`PIPELINES_QUERY_KEY` used by the task-create picker.

The management surface uses two read models:

- `GET /api/pipelines/statuses` is the inventory. It intentionally includes
  malformed `.toml` files and their parse line/column.
- `GET /api/pipelines/{id}` is the byte-preserving editor source. The UI sends
  this raw content unchanged to validation and write routes rather than parsing
  and reserializing TOML in the browser.

## File-id compatibility

Creating or overwriting a pipeline keeps the strict slug contract
(`A-Z`, `a-z`, `0-9`, `_`, `-`) because the id is also passed to pipeline
validation. Existing files discovered on disk can have safe non-slug stems such
as `foo.bar`; status inventory, raw read, and delete must still agree on those
ids. Existing-file operations therefore reject empty ids, `.`, `..`, path
separators, and NUL while allowing other safe stems. Do not loosen write/reset
validation to achieve this compatibility.

## Editor state and race controls

File-backed Settings editors have several asynchronous sources that must not
overwrite user input:

- Seed raw content only when the current host/id is unchanged and the draft is
  clean.
- Compare validation responses against the full host/id/content tuple before
  displaying them; a late response for an older draft is stale.
- After a successful write, pin the submitted content until the invalidated raw
  query returns that same value. Otherwise stale cached raw data can overwrite
  the just-saved editor.
- Keep a newly saved id selected until the refreshed status inventory contains
  it; choosing the first row too early makes a successful create appear to
  switch files.
- A raw-load error clears the prior selection's content and disables editing,
  preventing stale text from being saved under the failed id.

Reset-all only reseeds an open bundled file or an unsaved new draft that it
actually supersedes. It must not discard unrelated custom-file drafts.

## Settings host switching

Dirty-state confirmation is global because any Settings section may have
unsaved work, but host-specific cleanup belongs to the host-scoped subtree.
Keying that subtree by selected host remounts machine-specific sections after a
confirmed switch without clearing dirty state owned by universal Settings
sections. A cancelled switch leaves both the selected host and drafts intact.

