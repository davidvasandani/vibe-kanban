# Feature Specification: OAuth login for managed CLI tools

**Feature dir**: `specs/003-cli-tool-oauth-login/`
**Status**: Clarified (no open questions)
**Task**: `vk/5a2a-vk-cli-tool-logi`
**Scope (Constitution)**: local server CLI-tool service and shared settings UI.

## Summary

The CLI Tools settings page can install, update, and remove curated command-line
tools, but it sends users to vendor documentation when a tool still needs
credentials. Add an in-app Login action that starts the tool's own OAuth/device
login command in an interactive terminal, lets the user complete prompts or
browser authorization, reports the result, and refreshes authentication status.

The application orchestrates each vendor CLI; it does not collect, proxy, or
store OAuth access or refresh tokens itself. Credentials remain owned by the
CLI in its normal host-side configuration.

## User Stories

- As a user, I can see whether an available CLI tool is authenticated, so I know
  whether an agent can use it before starting work.
- As a user, I can click Login and complete the vendor's supported OAuth/device
  flow without finding and running the command in a separate terminal.
- As a user, I can see actionable output when login cannot start or fails, and I
  can retry without reinstalling the tool.
- As a remote-host user, I authenticate the selected machine's tool rather than
  accidentally changing credentials on the browser/UI machine.

## Functional Requirements

- FR-1: Each catalog entry MUST declare whether in-app login and authentication
  status detection are supported and, when supported, the commands needed for
  that specific tool.
- FR-2: Listing CLI tools MUST return an authentication state that distinguishes
  authenticated, unauthenticated, unknown/not-checkable, and unsupported.
- FR-3: For an available tool with supported login, the settings row MUST offer
  a Login action when it is not known to be authenticated and a Re-authenticate
  action when it is authenticated.
- FR-4: Starting login MUST execute the effective tool binary on the selected
  machine, respecting the existing host-copy-before-app-copy resolution rule.
- FR-5: Login MUST run in an interactive PTY-backed session whose output is
  streamed to the UI and whose input can be supplied by the user when the CLI
  prompts for choices or confirmation.
- FR-6: URLs emitted by the login command MUST be presented as clickable links;
  browser launch failure MUST NOT prevent the user from copying/opening the URL.
- FR-7: The user MUST be able to cancel an active login. The child process and
  PTY resources MUST be cleaned up after success, failure, cancellation, UI
  disconnect, or a 15-minute timeout.
- FR-8: Only one login session per tool and machine MAY be active at a time.
  A conflicting start MUST return a clear conflict response rather than start a
  second process.
- FR-9: After the login process exits, the service MUST re-run the tool-specific
  status probe and return the refreshed authentication state. Exit success alone
  MUST NOT be treated as proof of authentication.
- FR-10: Authentication probes MUST be bounded by a timeout and MUST NOT expose
  tokens, authorization codes, environment secrets, or credential-file contents
  in API responses or logs.
- FR-11: Unsupported tools MUST retain their documentation link and explain that
  login must be completed externally; tool installation behavior is unchanged.
- FR-12: Errors MUST distinguish at least unavailable binary, unsupported login,
  session conflict, timeout, cancellation, and command failure.
- FR-13: The interface MUST work through the existing machine-aware settings
  client for both the local machine and connected remote hosts.

## Out of Scope

- Implementing an OAuth authorization server or OAuth callback endpoint in Vibe
  Kanban.
- Reading, copying, encrypting, syncing, or deleting vendor credential files.
- Logging users out or revoking vendor tokens.
- Non-interactive service-principal, API-key, or workload-identity setup.
- Changing the curated CLI installation sources or PATH precedence.

## Acceptance Criteria

- [ ] An unauthenticated supported tool shows Login and its current auth state.
- [ ] Login opens an embedded interactive terminal and runs the catalog-declared
      command on the selected machine using the effective binary.
- [ ] The user can follow a displayed device/browser URL, respond to prompts,
      cancel the flow, and retry.
- [ ] Completion refreshes status using an independent probe; success is shown
      only when that probe confirms authentication.
- [ ] Concurrent login attempts for the same tool/machine are rejected clearly.
- [ ] Unsupported and unavailable tools never show a misleading Login action.
- [ ] Process output and server logs do not disclose OAuth tokens, authorization
      codes, environment secrets, or credential-file contents.
- [ ] Backend tests cover catalog metadata, auth-state mapping, effective binary
      resolution, lifecycle cleanup, conflicts, timeouts, and redaction.
- [ ] Frontend tests cover action visibility, terminal interaction, cancellation,
      refreshed status, and error states.
- [ ] Generated types, formatting, relevant Rust tests, frontend type checks, and
      lint pass.

## Clarifications

- Q-1: Which tools are supported initially? **Azure CLI** (`az login
  --use-device-code`, probed with `az account show`), **GAM7** (`gam oauth
  create`, probed with `gam oauth verify`), and **Microsoft Graph CLI beta**
  (`mgc-beta login`, probed with its non-secret status command). The exact Graph
  status arguments must be confirmed against the pinned binary during research.
- Q-2: Why not AWS CLI? AWS browser authentication is profile-specific and
  normally requires an existing IAM Identity Center profile (or an interactive
  `aws configure sso` setup). The app cannot safely infer which profile to
  configure or authenticate. AWS remains externally configured in this slice.
- Q-3: Why not 1Password CLI? `op signin` commonly creates a shell/process-scoped
  session value; a login subprocess cannot safely transfer that secret into
  future agent processes. Desktop-app integration is also host-policy dependent.
  1Password remains externally configured in this slice.
- Q-4: Which transport carries the interaction? Reuse the existing deployment
  PTY service and WebSocket terminal protocol, generalized so a catalog-owned
  command can start without a workspace checkout. Do not create a second PTY
  implementation.
- Q-5: What lifecycle applies? One session per tool/machine, a 15-minute maximum,
  explicit Cancel, and cleanup when the UI disconnects. A retry starts a fresh
  process and independently probes authentication again.
