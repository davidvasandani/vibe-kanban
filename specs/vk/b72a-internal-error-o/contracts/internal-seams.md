# Internal seams — `b72a-internal-error-o`

No HTTP route, wire type, or database column changes. The contracts below are
internal Rust seams; the only externally observable change is the body of one
error response.

## S1 — `GitCli::fetch_with_refspecs`

`crates/git/src/cli.rs`

```rust
pub fn fetch_with_refspecs(
    &self,
    repo_path: &Path,
    remote_url: &str,
    refspecs: &[&str],
) -> Result<(), GitCliError>
```

- Runs one `git fetch <remote_url> <refspec>…` with `GIT_TERMINAL_PROMPT=0` and
  the existing `classify_cli_error` mapping — the same behaviour
  `fetch_with_refspec` has today, generalised over the refspec count.
- `fetch_with_refspec` becomes a one-element delegation. Its three existing
  callers (`crates/git/src/lib.rs:1567` and two in
  `crates/git/tests/git_ops_safety.rs`) are unchanged and untouched.
- A wildcard refspec that matches nothing is **not** an error; git exits 0.
  Verified, and load-bearing: it is what lets the remote-tracking refspec share
  an invocation with the heads refspec without being able to fail it.

## S2 — `SharedRepositoryStore::resolved_branch_ref`

`crates/workspace-manager/src/shared_repository.rs`

```rust
fn resolved_branch_ref(
    cli: &GitCli,
    store: &Path,
    branch: &str,
) -> Result<Option<String>, WorkspaceError>
```

The single definition of "what ref does this target branch name in the store".

| input | store holds | returns |
| --- | --- | --- |
| `main` | `refs/heads/main` | `Some("refs/heads/main")` |
| `origin/main` | `refs/remotes/origin/main` | `Some("refs/remotes/origin/main")` |
| `shared` | both `refs/heads/shared` and `refs/remotes/shared` | `Some("refs/heads/shared")` |
| `nope` | neither | `None` |

- Order is local, then remote — the outcome of `GitService::find_branch`
  (`crates/git/src/lib.rs:1410-1425`), which is what validated the user's choice
  in the first place.
- Presence is proven with `GitCli::commit_exists` (`cat-file -e <rev>^{commit}`),
  not `rev-parse`. Reusing `commit_exists` also keeps one fail direction for both
  ref forms: `GitCliError::CommandFailed` means absent, every other error
  propagates.
- `branch_commit_present` is reimplemented as `…?.is_some()` and keeps its
  signature, so `ensure`, `store_resolves`, `adopt` and `mirror_branch_back` are
  unchanged at the call site.

## S3 — `SharedRepositoryStore::fallback_refspec`

`crates/workspace-manager/src/shared_repository.rs`

```rust
fn fallback_refspec(remote_name: &str, target_branch: &str) -> String
```

Pure; truth-table tested.

| remote | target branch | refspec |
| --- | --- | --- |
| `origin` | `origin/main` | `+refs/heads/main:refs/remotes/origin/main` |
| `origin` | `origin/release/1.x` | `+refs/heads/release/1.x:refs/remotes/origin/release/1.x` |
| `origin` | `main` | `+refs/heads/main:refs/heads/main` |
| `upstream` | `origin/main` | `+refs/heads/origin/main:refs/heads/origin/main` |

The last row is deliberate: when the target branch is not prefixed with *this*
remote's name, nothing about it says the remote has a branch by another name, so
the existing local-to-local form is kept rather than guessed at.

## S4 — mirroring refspecs

`publish_and_fetch` fetches, best-effort, from the registered checkout:

```
+refs/heads/*:refs/heads/*
+refs/remotes/*:refs/remotes/*
```

Additive: force-update, never `--prune`. Runs inside the repository
administration lease, after `configure()` and the rename, so
`core.sharedRepository=group` is already in effect for the ref files it writes.

## S5 — the error channel

One new variant at each layer, and one new arm in the existing `From`/render
matches. Nothing else changes shape.

```rust
// crates/workspace-manager/src/workspace_manager.rs
#[error("shared repository store for '{repo_name}' cannot serve branch '{branch}': {detail}")]
WorkspaceError::SharedStore { repo_name: String, branch: String, detail: String }

// crates/services/src/services/container.rs
#[error("{0}")]
ContainerError::SharedStore(String)

// crates/server/src/error.rs
ApiError::ClusterProvisioning(String)
  => ErrorInfo::with_status(INTERNAL_SERVER_ERROR, "ClusterProvisioningError", msg)
```

- The `From<ContainerError> for ApiError` arm is added **before** the
  `other => ApiError::Container(other)` catch-all.
- Status stays 5xx so `error.rs`'s `is_server_error()` branch still emits the
  `tracing::error!` record.
- `ApiError` is `#[ts(type = "string")]`, so `shared/types.ts` does not change
  and no type regeneration is required.

### Observable response change

Only for this one failure:

```diff
  HTTP 500
- {"success":false,"message":"An internal error occurred. Please try again.", …}
+ {"success":false,"message":"shared repository store for 'homelab' cannot serve
+  branch 'origin/main': …", …}
```

Rendered verbatim by `CreateChatBoxContainer`'s `displayError`, which already
shows `error.message` unmodified.
