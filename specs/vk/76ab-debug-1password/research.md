# Research

## Incident evidence (2026-10-15)

- SWE-278 session `ca4f7dc8…`: "Failed to start execution: 1Password lookup
  failed…" at 03:55:16, 03:55:26 and 05:09:46 UTC. The turn at 10:14 UTC
  started normally.
- The Sweetgreen org (which owns project "Platform Ops") has one reference,
  `GH_TOKEN = op://Homelab/alderbridge nix PAT/credential`, and its own
  literal `OP_SERVICE_ACCOUNT_TOKEN`.
- With that org token, run now: `op whoami` → service account on
  `vasandani.1password.com`; vaults: `Homelab`; `op read <ref>` → exit 0.
- `op service-account ratelimit`: `account read_write 1000/24h, 91 used,
  reset in 21 h` (window opened about 07:20 UTC), plus `token read 1000/h`.

Conclusion: the configuration is valid, and quota exhaustion is the most
likely cause. It cannot be proven, because stderr was discarded. That gap is
what this feature closes.

## Observed `op` 2.34.1 stderr

| Situation | stderr (abridged) | Cause |
|---|---|---|
| Missing item | `"x" isn't an item in the "Homelab" vault` | NotFound |
| Missing vault | `"V" isn't a vault in this account` | NotFound |
| Missing field | `item '…' does not have a field 'f'` | NotFound |
| Garbage token | `error initializing client: … failed to DecodeSACredentials …` | Unauthorized |
| Unwritable HOME | `cannot create directory "…/.config/op" … permission denied` | Unknown |
| Rate limit (1Password docs) | `Too many requests` / HTTP 429 / "rate limit" | RateLimited |

## Alternatives considered

- **Surface sanitised stderr.** Rejected. The stderr echoes the reference,
  vault and item names, and any future CLI text, so no redaction rule is
  provably complete (LV, homelab 13).
- **Cache resolved values (like the 60-second owner-token cache).** Deferred.
  It saves quota but extends the lifetime of secrets in memory, and it needs
  its own decision.
- **Retry on rate limit.** Rejected (see the spec's clarifications).
- **Add a `variable` field via a wrapper variant.** Rejected. A field on
  `ReadFailed` keeps every existing `match` simple.
