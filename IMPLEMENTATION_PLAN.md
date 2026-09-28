# Implementation plan — vk/2eb6-keep-env-vars-un

See `SPEC.md` and `PRIOR_KNOWLEDGE.md`.

1. **Shared rule (Rust).** Add `normalize_secret_reference(&str) -> Option<String>`
   to `crates/api-types/src/organization_env_var.rs`, with unit tests: bare,
   whitespace, each quote style, mismatched quotes, quoted literal, empty
   quotes, and case sensitivity.
2. **API type.** Add `reference: Option<String>` (`#[serde(default)]`,
   `#[ts(optional)]`) to `OrganizationEnvVar` and drop its `sqlx::FromRow`
   derive if nothing needs it.
3. **Remote repository.** Add a private `OrganizationEnvVarRow` and point every
   `query_as!` at it, with the SQL unchanged. Map rows to `OrganizationEnvVar`
   with `reference: None`. In the list handler, reuse the existing
   `list_with_encrypted_values` (keyed by name, which is unique per org) rather
   than adding a new query.
4. **Remote routes.** Normalize on create/update before validation and
   encryption. Fill `reference` in the list (decrypt and normalize) and in the
   create/update responses (from the normalized value).
5. **Resolver.** Use the shared rule in `environment_secrets` for
   `has_references`, `select_token`, and `resolve_with_binary`. Add tests for
   a quoted legacy value and for rejecting a quoted token.
6. **Types.** Run `pnpm run generate-types` to regenerate `shared/types.ts`.
7. **UI helper.** Add `secretReference.ts` (`normalizeSecretReference`) and a
   Vitest spec.
8. **Card.** Show the reference in rows, prefill edit, normalize the draft on
   change, pick the input type from the normalized draft, and update the
   description. Extend `OrganizationEnvVarsCard.test.tsx` with the pasted
   quoted reference, the displayed saved reference, a literal that stays masked,
   and the prefilled edit.
9. **Docs.** Update `docs/settings/organization-settings.mdx` if it describes
   masking.
10. **Verify.** Run `cargo test -p api-types -p services`, `cargo check -p remote`
    (via `pnpm run backend:check`), `pnpm run check`, the web-core vitest suite,
    `generate-types:check`, and `pnpm run format`.
