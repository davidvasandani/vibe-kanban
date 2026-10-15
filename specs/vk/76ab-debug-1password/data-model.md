# Data model

No persisted entities change. In-memory error type
(`crates/services/src/services/environment_secrets.rs`):

```rust
pub enum LookupFailure { RateLimited, Unauthorized, NotFound, Unknown }

pub enum EnvironmentSecretError {
    MissingToken,
    MissingCli,
    ReadFailed { variable: Option<String>, cause: LookupFailure },
    TimedOut,
    InvalidValue,
}
```

- `variable` is an env var **name** (never a value or a reference). It is
  `None` for single-reference callers, which name the owner themselves.
- `cause` is derived from at most 8 KiB of stderr, which is then dropped.
