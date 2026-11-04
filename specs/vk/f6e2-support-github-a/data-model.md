# Data model and contracts

Existing github_owner_tokens rows and encryption AAD remain unchanged. Legacy plaintext is a PAT or PAT reference. A versioned App envelope contains app_id, installation_id and private_key_reference. IDs are positive decimal strings.

Create/update requests retain value for PATs and add optional app metadata. App submissions reject simultaneous nonempty PAT values. Views return optional app metadata and never keys or tokens. Health adds optional App evidence with app_reachable, installation_found, token_minted and missing_permissions. Existing status values remain ok/rejected/unavailable.

Cached tokens are memory-only and keyed by owner, row id and encrypted revision. Expiry is the earlier safe reuse deadline derived from GitHub expires_at. Minting is single flight; invalidation prevents stale cache insertion.

Launch capabilities are authenticated encrypted payloads bound to row id, credential revision and seven-day expiry. Broker responses are secret-bearing and must use Cache-Control: no-store. Invalid, expired, changed or deleted credentials refuse refresh. Broker does not accept arbitrary owner IDs as authority.
