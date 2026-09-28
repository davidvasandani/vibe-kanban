# SPEC — Keep 1Password reference Env Vars unobfuscated

Task: `vk/2eb6-keep-env-vars-un`

## Problem

Organization Settings → Environment variables shows every saved value as
`••••••••`. That includes values that are only 1Password references. A reference
such as `op://Homelab/alderbridge nix PAT/credential` is a pointer, not a
secret. Hiding it means the operator cannot see which item a variable points to.

1Password's "Copy secret reference" puts double quotes around the value on the
clipboard: `"op://Homelab/alderbridge nix PAT/credential"`. PR #324 only shows
plain text for a draft that starts with `op://` exactly, so a pasted value
starts with `"` and stays masked. It is also saved with the quotes, and the
resolver (`services::environment_secrets`) only treats values starting with
`op://` as references. A quoted value is therefore injected into agents
**literally**, and `GH_TOKEN` gets set to the quoted reference string instead of
the resolved PAT.

## Goals

1. **Saved references are readable.** In the Env Vars list, a variable whose
   stored value is a 1Password reference shows that reference in plain text.
   Literal values stay masked.
2. **Pasted quotes are removed.** When a whole value is a 1Password reference
   wrapped in one pair of matching quotes (`"…"`, `'…'`, or curly `“…”`/`‘…’`),
   with optional surrounding whitespace, it is stored as the bare reference.
   The same rule applies to both draft inputs, so a pasted quoted reference is
   shown as text immediately.
3. **Values already saved with quotes work.** A stored `"op://…"` value is
   resolved as a reference when the environment is prepared, and the list shows
   it unquoted. The operator does not need to re-enter it.
4. **Editing a reference is easy.** The edit field for a reference variable
   starts with the current reference, not an empty field.

## Non-goals

- Revealing literal (non-reference) secret values in any form.
- Changing the reference syntax, how references are resolved, or the token
  precedence.
- Removing quotes from literal values. Quotes may be part of a real secret, so
  only a quoted whole-value `op://` reference is normalized.

## Design

### Normalization rule (one definition per language)

`normalize_secret_reference(value) -> Option<String>`: trim ASCII/Unicode
whitespace. If the result is wrapped in one matching quote pair (`"` `"`, `'` `'`,
`“` `”`, `‘` `’`), remove the pair and trim again. If what remains starts with
`op://`, return it. Otherwise return `None`, and the caller keeps the original
value unchanged, byte for byte.

- Rust: in `api-types` (a crate both `remote` and `services` depend on), so the
  write path and the resolver use the same rule.
- TypeScript: a small pure helper next to the card, `secretReference.ts`, with
  Vitest coverage.

### Write path (remote)

`create_env_var` / `update_env_var` normalize the value before validation and
encryption. A quoted reference is stored bare. Any other value is stored
exactly as sent.

### Read path (remote list)

`OrganizationEnvVar` gets an optional `reference: Option<String>` field
(ts: `reference?: string | null`, serde default so older payloads still
deserialize). `list_env_vars` decrypts each row. It fills `reference` only when
`normalize_secret_reference` returns `Some`. Literal values are never put in a
response. Create and update responses fill it in the same way from the value
just written. The list endpoint stays admin-only, as it is today.

The SQL text in the repository does not change. `query_as!` targets a private
`OrganizationEnvVarRow` that is then mapped into the API type, so the SQLx
offline cache stays valid.

### Resolver (services)

`environment_secrets` uses the shared rule to detect and read references. A
legacy quoted value then resolves to the secret. A literal
`OP_SERVICE_ACCOUNT_TOKEN` is still rejected when it is (or normalizes to) a
reference.

### UI

- Row: when `envVar.reference` is set, show it in monospace text instead of
  dots, with a `title` so long values can be read in full.
- Draft inputs (add and edit): `type` is text when the normalized draft is a
  reference. On paste or change, a whole-value quoted reference is replaced by
  the bare reference.
- Edit prefills `envVar.reference ?? ''`.
- The card description states that quotes copied from 1Password are removed.

## Acceptance criteria

- Pasting `"op://Homelab/alderbridge nix PAT/credential"` into the add input
  shows `op://Homelab/alderbridge nix PAT/credential` in a text input, and the
  create request carries the bare reference.
- The saved row shows the reference in plain text. A literal row still shows
  `••••••••`.
- A remote create/update with a quoted reference stores the bare reference, and
  a literal `"quoted literal"` is stored unchanged.
- The resolver treats a stored `"op://v/i/f"` as reference `op://v/i/f`.
- `pnpm run check`, the web-core Vitest suite, the `api-types`/`services`/`remote`
  Rust tests, and `generate-types:check` pass.
