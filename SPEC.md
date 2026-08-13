# Technical Specification: Low-Disk Warnings and Issue Follow-Through

## Goal

Make dangerous filesystem pressure immediately visible in the existing Server
Metrics accordion and give an operator a one-click, duplicate-safe path to an
actionable Vibe Kanban issue that targets permanent remediation.

## Scope

This change is limited to the Vibe Kanban service source and, where deployment
configuration is required, `homelab/modules/vibe-kanban-rebuild.nix`. It does
not gate scheduling or change any other hosted service.

## User experience

- Evaluate every filesystem sample for every visible node against configurable
  warning and critical thresholds.
- A filesystem is warning when either its free percentage or free byte count is
  below the warning boundary. It is critical when either is below the critical
  boundary. Critical takes precedence.
- Default boundaries are warning below 10% free or below 5 GiB, and critical
  below 2% free or below 1 GiB. Equality is not below the boundary.
- Affected server rows show a warning-triangle icon, a textual severity label,
  and theme-safe warning/critical styling. The concrete filesystem, available
  capacity, usage percentage, and mountpoint are visible without requiring a
  chart reading.
- The accordion header rolls up the worst current severity and affected-node
  count so pressure remains visible while collapsed.
- Activating a warning by mouse or keyboard opens the matching existing open
  low-disk issue, or creates a pre-filled issue when none exists.
- The issue includes node ID and hostname, observation timestamp, filesystem,
  mountpoint, size, used, available, and use percentage. Its remediation prompt
  asks for root-cause analysis, sustainable garbage collection/retention, and a
  volume-sizing decision rather than only immediate cleanup.
- While issue lookup/creation is pending, repeated activation is disabled. A
  server-side idempotency guard ensures concurrent or repeated requests cannot
  create two open low-disk issues for the same node.

## Configuration

Expose four service settings, wired through the existing configuration pattern:

- warning free percent: `10`
- warning free bytes: `5 GiB`
- critical free percent: `2`
- critical free bytes: `1 GiB`

Configuration must validate that values are non-negative and that each critical
boundary is no less severe than its warning counterpart. The effective values
must be available to the UI from the backend rather than duplicated as frontend
constants. Defaults must be documented for operators.

## Data and API behavior

Use current node-metrics samples as the source of disk facts. Add a narrowly
scoped coordinator endpoint/action for resolving a low-disk issue. The request
identifies the node and filesystem sample/observation being acted on; the server
validates the referenced current metrics and derives canonical issue content.
The response distinguishes `created` from `existing` and returns the issue ID.

Persist a machine-readable low-disk identity with the issue (or in a dedicated
association) keyed by node ID. Duplicate detection considers only open issues;
after the prior issue has a completion timestamp or uses the established
Done/Cancelled/Canceled status, a new incident may create a new issue. The
database must enforce the open-issue uniqueness invariant rather than relying
only on a title search.

## Accessibility and failure states

Severity is never communicated by color alone. Interactive warnings are native
buttons or links with descriptive accessible names and visible focus treatment.
Issue-resolution failures leave metrics visible, show a recoverable error, and
allow retry. Missing or malformed metrics do not produce false warnings.

## Verification

- Unit tests cover threshold boundaries, the OR (more conservative) rule,
  severity precedence, formatting, and rollup counts.
- Component tests cover collapsed-header visibility, icon/text treatment,
  concrete disk facts, keyboard activation, pending behavior, existing-issue
  navigation, and recoverable failures in light/dark-compatible class usage.
- Backend tests cover issue content, open-issue reuse, concurrent idempotency,
  closed-issue recreation, validation, and authorization.
- Configuration tests cover defaults, overrides, and invalid ordering.
- Existing Server Metrics and issue flows remain green.

## Out of scope

- Automatically deleting data or resizing volumes.
- Preventing the scheduler from dispatching work to a pressured node.
- Alert delivery outside the Server Metrics UI.
- Changes to services other than Vibe Kanban.

