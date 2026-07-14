# Research: CLI OAuth login

## Vendor command decisions

| Tool | Login | Probe | First release | Reason |
|---|---|---|---|---|
| Azure CLI | `az login --use-device-code` | `az account show --output none` | Yes | Stable device flow; avoids relying on browser launch on the host. |
| GAM7 | `gam oauth create` | `gam oauth verify` | Yes | Vendor-documented interactive scope selection and OAuth creation/verification. |
| Microsoft Graph CLI beta | `mgc-beta login` | Confirm non-secret status subcommand against pinned binary | Conditional | Login is supported; implementation must lock the exact probe before enabling metadata. |
| AWS CLI | none automatically | profile-specific `aws sts get-caller-identity` | No | SSO login requires a chosen/configured profile; generic state would be misleading. |
| 1Password CLI | none | host-policy/session dependent | No | Sign-in commonly produces process-scoped session material that cannot safely carry to agents. |

## Architecture decisions

- Use a PTY because GAM is genuinely prompt-driven and device flows can still
  prompt for account/subscription choices.
- Start vendor binaries directly, not through a shell string. Executable and
  arguments come exclusively from the compiled catalog.
- Reuse the signed WebSocket and xterm protocol. Add typed `exit` and `status`
  messages rather than parsing terminal text in React.
- Authentication state is observational and ephemeral. A failed probe means
  `unauthenticated` only when the tool's documented exit contract makes that
  conclusion safe; otherwise it means `unknown`.
- Do not store or replay terminal transcripts.

## Sources consulted

- AWS CLI IAM Identity Center configuration and profile-specific login guidance.
- AWS STS `get-caller-identity` command reference.
- Microsoft Azure CLI interactive/device-code login and `az account show` docs.
- GAM7 installation/authorization wiki (`gam oauth create`, `gam oauth verify`).
- Microsoft Graph CLI upstream project/help for login/status command validation.
