# Research

- Existing knowledge: workspace-root PRIOR_KNOWLEDGE.md, wiki/github-owner-token-routing.md, wiki/1password-reference-resolution.md.
- GitHub's List check runs for a Git reference contract lists App user and installation tokens with Checks read permission: https://docs.github.com/en/rest/checks/runs#list-check-runs-for-a-git-reference . Retrieved 2026-11-04 with Firecrawl.
- App JWTs use RS256, iat with clock-skew allowance and exp within ten minutes. Installation tokens return expires_at, which must govern reuse: https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-an-installation-access-token-for-a-github-app . Verify full contract before coding.
- jsonwebtoken 10.3.0 already exists in Cargo.lock. Reuse it rather than invoking openssl with keys in subprocesses.
- environment_secrets accepts multiline UTF-8 without NUL; PEM can use its existing bounded resolution.
- Launch-only tokens cannot satisfy long sessions. A scoped refresh capability keeps the App key away from agents and works across workers. A failed refresh must never expose a cached expired token or select ambient authentication.
- Admin bypass is deferred. No ruleset or per-author exception is needed for credential support.
