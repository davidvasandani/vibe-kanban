# Data model

There are no persistence changes. `github_owner_tokens` (id, owner NOCASE
unique, encrypted_value, created_at, updated_at) is reused as is.

## In-memory types (`utils::github_credentials`)

| Type | Fields | Notes |
|---|---|---|
| `GitHubRepoRef` | `owner: String`, `repo: String` | Parsed from a remote or PR URL on github.com |
| `GitHubToken` | `Arc<str>` | `Debug` prints `<redacted>` |
| `OwnerCredential` | `OrgToken(GitHubToken)` \| `Unavailable(String)` | The reason is non-secret `Display` text |
| `GitHubCredentials` | `HashMap<lowercase owner, (owner, OwnerCredential)>` | `Default` means empty (all fallback) |
| `CredentialSelection` | `OrgToken{owner,token}` \| `Unavailable{owner,reason}` \| `Fallback{owner: Option<String>}` | Result of a lookup by owner or URL |

## In-memory cache (`services::github_credentials`)

`(row id, updated_at, reference) → (resolved_at, GitHubToken)`, with a 60 s
TTL, cleared by Settings writes. It is process-local and never persisted.
