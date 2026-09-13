# Remove obsolete private-deployment workflow

## Background

The repository currently runs two deployment dispatch workflows on pushes to
`main`. The legacy `Trigger Deployment` workflow targets a private deployment
repository and requires a token that is no longer configured, so it produces a
false failing CI result. `Trigger homelab CD` is the supported deployment path.

## Requirements

- Delete the legacy `.github/workflows/trigger-fork-deploy.yml` workflow.
- Preserve `.github/workflows/trigger-homelab-deploy.yml` behavior: pushes to
  `main` and manual runs dispatch `vibe-kanban-deploy` to
  `davidvasandani/homelab`, with `github.sha` and `github.ref_name` in the
  client payload.
- Leave `.github/workflows/test.yml` unchanged.
- Remove documentation describing the retired private deployment workflow and
  replace it with a concise description of the active homelab dispatch.
- Make no changes to the homelab repository or any other service.

## Validation

- Confirm the obsolete workflow file is absent.
- Search the repository for the retired repository target, workflow name, and
  token secret.
- Parse all remaining GitHub Actions workflow YAML.
- Assert the homelab workflow's push trigger, repository, event type, token,
  and SHA/ref payload are unchanged from the pre-change version; explanatory
  comments may be updated to remove stale references.
- Assert the standard Test workflow is byte-for-byte unchanged.

## Non-goals

- Changing the homelab deployment workflow or its infrastructure.
- Changing application code, dependencies, tests, or other services.
- Removing the active `HOMELAB_DEPLOY_TOKEN` requirement.
