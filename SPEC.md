# Technical specification: Codex Slack MCP and Azure/Entra capabilities

**Task:** `vk/84ef-restore-slack-mc`

## Objective

Make the Slack connector and the Azure CLI-backed Microsoft Entra read path that
an operator enables for Codex actually available inside Vibe Kanban Codex
workspaces. Availability must be consistent for fresh sessions and must be
refreshable for a workspace whose agent process started before a connector was
enabled.

## Scope

Changes are limited to the Vibe Kanban application and its deployment/runtime
configuration in `homelab/modules/vibe-kanban-rebuild.nix`. The work may update
Codex executor configuration, workspace process lifecycle and diagnostics,
deployment packages and runtime authentication wiring, focused tests, and Vibe
Kanban documentation. It must not change Slack, Entra, LogMeIn, Automox, or any
other hosted service, and validation operations must remain read-only.

## Required behavior

1. A Slack MCP definition connected and assigned to Codex is included in the
   native Codex MCP configuration read by every newly started workspace agent.
   It exposes a read-only Slack message-search tool capable of exact-hostname
   searches.
2. The workspace execution environment includes an `az` executable on `PATH`.
3. The workspace receives a non-secret-bearing Azure authentication context
   suitable for `az account show` and read-only Microsoft Graph device queries.
   Credentials remain in the existing runtime secret boundary and are never
   copied into repositories, prompts, logs, or diagnostics.
4. A supported refresh action restarts the agent process for an existing task,
   preserves task/chat continuity, and reloads the current native MCP
   configuration. Enabling or reconnecting Slack therefore does not require
   recreating the task or workspace.
5. The UI reports a useful mismatch diagnostic when its configured/connected
   Slack state does not agree with the MCP definition or runtime capability
   visible to the selected Codex executor. Azure diagnostics distinguish a
   missing executable, missing runtime auth context, and failed account/Graph
   access without disclosing sensitive values.

## Security and operational constraints

- Slack and Entra validation is read-only; no messages, users, groups, devices,
  memberships, or inventory records are created, modified, or deleted.
- Slack tokens and Azure client credentials are loaded through protected
  runtime credential files or equivalent opaque references, never emitted as
  environment values visible in diagnostics.
- Microsoft Graph permissions are the minimum application/delegated read scopes
  required for exact device lookup.
- Refresh affects the active agent process only after the current turn exits
  safely; it must not discard Vibe Kanban task history.
- Existing custom MCP definitions must not be silently overwritten.

## Acceptance criteria

- In a fresh Codex workspace, the connected Slack MCP exposes a read-only
  message-search tool and an exact search for each validation hostname can run.
- In that workspace, `command -v az` returns an executable path and
  `az account show` succeeds without secrets in output or logs.
- A read-only Graph/Entra device lookup by exact hostname succeeds.
- Reconnecting or enabling Slack after workspace creation becomes visible after
  the supported agent refresh action, without recreating the task.
- A regression test covers a user-visible enabled/connected state whose agent
  tool registry or runtime capability is absent and verifies the diagnostic.
- Documentation states the required Slack connection and permissions, Azure
  authentication mechanism and Graph read permissions, refresh procedure, and
  troubleshooting checks.
- The four hostnames `USSG01RG0300221`, `USSG01RG0200047`,
  `USSG01RG0100047`, and `USSG01PM0100047` can be searched read-only across
  Slack and Entra and correlated with the already available LogMeIn MCP.

## Open questions for clarification

- Which existing deployment-owned Azure identity and secret source is intended
  for the read-only Entra device lookup?
- Whether the mismatch diagnostic should be computed from configuration and
  executor-native state only, or additionally perform bounded live probes.
- Whether Azure CLI access is required on coordinator and all worker roles, or
  only hosts that execute Codex workspaces.

## Non-goals

- Automox cleanup or any mutation of inventory candidates.
- Changes to LogMeIn MCP behavior.
- General-purpose Azure administration or broad Microsoft Graph permissions.
- Hot-injecting new tools into a currently running Codex process without a safe
  process restart.
