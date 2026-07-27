# Clarifications: verified Slack MCP installation

Task: `95e9-close-the-unveri`

## Q1. Can this task publish the launcher to a fork-controlled npm package?

No. On 2026-07-27:

- `npm view @davidvasandani/slack-mcp-server-vk ...` returned `E404`;
- `npm whoami` returned `ENEEDAUTH`.

The absence of local credentials does not prove that no maintainer owns the npm
scope, but it does prove this task cannot safely create or publish the proposed
package. Namespace claiming and credential creation are external, authorised
operations, not repository implementation details.

## Q2. Should the task use VK's managed CLI tool catalogue instead?

Not in this slice. That catalogue is an explicit per-user vendor-tool management
surface. Using it for a default MCP entry would require:

- a user-visible prerequisite before adding the suggested server;
- a stable executable-path expansion rather than a static self-installing
  command;
- platform-specific sources and unsupported-host behaviour;
- install/update/removal state coupled to MCP suggestions;
- a decision about host-copy precedence, which is inappropriate for a connector
  that must run the reviewed fork build.

Those changes are feasible but materially broader than the one-line npm
transition that becomes available once publication authority exists. Shipping a
static entry that merely assumes the managed executable exists would regress
clean-user behaviour and fail FR-7.

## Q3. What posture ships now?

An explicit temporary detect-only exception:

- preserve the exact GitHub release URL and recorded outer tarball SHA-256;
- run the digest audit daily instead of weekly;
- on audit failure, open or update a durable GitHub issue through a
  least-privilege `GITHUB_TOKEN`;
- document that a release writer can still replace the launcher and its inner
  digest table before detection;
- retain the rule that corrections publish a new `-vk.<n+1>` tag.

This does not satisfy pre-execution prevention and must not be represented as
doing so. It satisfies the specification's explicitly permitted exception path.

## Q4. What reopens prevention?

Maintainers obtaining a fork-controlled npm package name and configuring trusted
publication for the launcher. The follow-up then pins exact `name@version`,
checks the packument's `dist.integrity`, updates the digest/source constants and
tests, changes integration and knowledge documentation, and switches Renovate
to the npm datasource in one reviewed change.

