# Feature Specification: Verified Slack MCP installation

**Feature dir**: `specs/vk/95e9-close-the-unveri/`
**Status**: Draft

## Summary

Protect users of the bundled Slack MCP connector from executing a replacement
launcher before its provenance has been checked. The current fork launcher
verifies the platform binary it downloads, but its own GitHub release tarball is
downloaded and executed by npm without an expected integrity value. This
feature either moves that outer package to a delivery mechanism that verifies it
before execution or records an explicit, bounded decision to retain
detection-only controls until a named prerequisite becomes available.

## User Stories

- As a user enabling the bundled Slack connector, I want its first launch to
  verify the delivered code before running it so that control of a mutable
  release asset is not enough to execute arbitrary code on my machine.
- As a maintainer, I want the delivery pin, integrity record, documentation, and
  dependency-update automation to describe one artefact so that upgrades cannot
  silently weaken the control.
- As a security reviewer, I want prevention and detection controls labelled
  accurately so that accepted residual risk has an owner and a reopening
  condition.
- As an agent user, I want the pinned fork's attachment tools to remain
  available after the delivery change so that supply-chain hardening does not
  regress functionality.

## Functional Requirements

- FR-1: The bundled Slack connector must identify an exact released revision
  from the same fork advertised in its catalogue metadata.
- FR-2: Before first execution, the outer delivery artefact must pass an
  integrity or authenticity check anchored outside that artefact.
- FR-3: The platform executable must continue to pass its own exact digest check
  before execution.
- FR-4: Any verification failure must stop launch with an actionable diagnostic;
  the product must not fall back to an upstream package, another release, or a
  host executable of unknown provenance.
- FR-5: The canonical Slack definition must preserve stdio transport and the
  `SLACK_MCP_XOXP_TOKEN` placeholder across supported agent adaptations.
- FR-6: The selected delivery mechanism must work on every platform supported
  by the fork release, or clearly identify unsupported platforms before a user
  attempts launch.
- FR-7: A missing required installation step must be visible and actionable
  before the user relies on the generated connector definition.
- FR-8: The source revision and integrity metadata must have one documented
  update procedure. Automated dependency proposals must update or explicitly
  call out every coupled value and require human review.
- FR-9: User-facing integration documentation must explain where the connector
  comes from, which layer verifies it, and any prerequisite installation step.
- FR-10: Project knowledge must distinguish enforced inner-binary verification,
  enforced or absent outer-package verification, and scheduled audit detection.
- FR-11: A clean-cache end-to-end launch must expose `attachment_get_data` in
  `tools/list` and successfully retrieve a real attachment when the configured
  Slack workspace supplies a valid token and attachment.
- FR-12: If no preventative mechanism is currently deliverable, the repository
  must contain an explicit decision accepting detection-only risk. It must name
  the unavailable prerequisite, explain why managed installation is not chosen,
  state the maximum detection window and notification path, and define the
  concrete event that reopens the decision.
- FR-13: A detect-only exception must retain an automated published-artefact
  digest audit and must not be described as closing or preventing the attack.

## Out of Scope

- Changing Slack MCP tool semantics or Slack authentication.
- Treating an inner signature check as protection for an already-executing
  unverified launcher.
- Publishing to or claiming an npm namespace without demonstrated authority.
- Generalising every preconfigured MCP entry into an app-managed tool.
- Replacing the coding agents' own MCP process management.

## Acceptance Criteria

- [ ] The predecessor pinned-fork baseline is present and its immutable-source
  catalogue test passes.
- [ ] A clean first launch verifies the outer artefact before executing
  fork-controlled code, or a committed decision record satisfies FR-12 and
  FR-13.
- [ ] The expected Slack definition and its Codex and Opencode adaptations pass
  focused tests.
- [ ] The integrity source-of-truth and integration documentation move with any
  delivery change.
- [ ] Renovate validates and tracks the selected package/release source without
  automerging updates that require refreshed integrity metadata.
- [ ] With empty npm and launcher caches, the connector completes the MCP
  handshake and lists `attachment_get_data`.
- [ ] With a valid Slack token and real attachment fixture, calling
  `attachment_get_data` returns that attachment; if those external fixtures are
  unavailable to CI, the manual verification contract and exact prerequisite
  are documented.
- [ ] Repository formatting and focused backend checks pass.
- [ ] Independent review reports no significant unresolved findings.

## Clarified Decisions

- The proposed fork-controlled npm package does not exist and this environment
  has no npm publication identity (`npm whoami` returns `ENEEDAUTH`). This task
  must not claim or publish a namespace without maintainer authority.
- This task accepts the detect-only posture temporarily instead of adding an
  app-managed installation flow. The managed flow would add a per-user install
  prerequisite, UI/API lifecycle, platform mapping, and a dynamic executable
  path contract to a catalogue whose entries are currently self-installing.
  That product and architecture expansion is disproportionate while the
  smallest preventative path remains blocked only on npm ownership.
- The scheduled audit will run daily. On failure, a repository-native workflow
  step will open or update a GitHub issue using the workflow's scoped
  `GITHUB_TOKEN`; this requires no new third-party credential and gives the
  failure an explicit durable notification path.
- The prevention decision reopens when maintainers obtain a fork-controlled npm
  package name and configure trusted publication for the launcher. At that
  point the bundled entry moves to exact `name@version` delivery and the
  registry's `dist.integrity` becomes the pre-execution outer-package control.

## Open Questions

None.
