# Implementation plan: say which 1Password lookup failed, and why

See `SPEC.md`. All code is in
`crates/services/src/services/environment_secrets.rs`, plus two call-site
adjustments.

1. **Cause type.** Add `pub enum LookupFailure { RateLimited, Unauthorized,
   NotFound, Unknown }` with `Display` advice text. Change
   `EnvironmentSecretError::ReadFailed` to
   `ReadFailed { variable: Option<String>, cause: LookupFailure }`, with
   Display `1Password lookup[ for NAME] failed: <advice>`.
2. **Classifier.** `fn classify(stderr: &[u8]) -> LookupFailure`: lossy-lowercase
   the text and test the needle groups in the order rate limit → unauthorized
   → not found → unknown.
3. **Reader.** In `read_command`, set `stderr(Stdio::piped())`. In
   `read_reference`, read stdout (64 KiB + 1 bound) and stderr (8 KiB bound)
   concurrently with `tokio::join!`, then drop both pipes before `wait()`.
   Spawn and IO errors → `Unknown`. On a non-zero exit → `classify(stderr)`.
   The stderr buffer is never returned or logged.
4. **Name the variable.** In `resolve_with_binary`, iterate `(key, value)`
   and map `ReadFailed { variable: None, .. }` to `Some(key)`. Log a
   `tracing::warn!` with variable and cause.
5. **GitHub paths.** Add `EnvironmentSecretError::without_variable(self)`.
   Apply it in `github_credentials::resolve_reference` and in
   `github_owner_tokens::launch_environment` so the internal `VALUE` key is
   never shown.
6. **Fix existing `ReadFailed` constructors** in the tests of
   `github_credentials.rs` and `github_owner_tokens.rs`.
7. **Tests** (in `environment_secrets.rs`):
   - Classifier table using real `op` 2.34 messages (not an item, not a
     vault, no field, DecodeSACredentials, 429/too many requests).
   - A fake `op` printing a rate-limit message on stderr → `RateLimited`, the
     variable is named, and the message contains no stderr text.
   - The existing privacy test still holds, with the cause `Unknown`.
   - `without_variable` drops the name.
8. **Docs.** Add a 1Password troubleshooting note (failure causes and quota)
   to `docs/` where org Env Vars are documented.
9. **Verify.** `cargo test -p services environment_secrets github_`,
   `cargo clippy -p services -p server`, `pnpm run format`.
