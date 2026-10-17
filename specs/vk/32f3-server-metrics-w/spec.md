# Feature Specification: Server Metrics Low-Disk Warnings

**Feature dir**: `specs/vk/32f3-server-metrics-w/`
**Status**: Draft

## Summary

Server Metrics must call attention to filesystems that are running out of free
space and let an operator turn the observed facts into a durable remediation
issue in one action. The warning must remain visible while the accordion is
collapsed, distinguish warning from critical pressure accessibly, and avoid
creating duplicate open incidents for the same node.

## User Stories

- As an operator, I want low disk space to be unmistakable in Server Metrics so
  I can recognize a host problem before agent failures look like build defects.
- As an operator, I want the worst disk state summarized on the collapsed
  accordion so I do not need to remember to inspect the panel.
- As an operator, I want exact `df -h`-style facts beside the warning so I can
  judge urgency without opening a shell.
- As an operator, I want one click to create or open a permanent-remediation
  issue so the condition is investigated and prevented from recurring.
- As an administrator, I want thresholds configurable so deployments with
  different volume sizes can choose appropriate operating margins.

## Functional Requirements

### Classification and presentation

- FR-1: The system MUST classify each valid current filesystem reading as
  normal, warning, or critical.
- FR-2: By default, warning means less than 10% free **or** less than 5 GiB
  free, whichever detects pressure sooner.
- FR-3: By default, critical means less than 2% free **or** less than 1 GiB
  free, whichever detects pressure sooner.
- FR-4: A value exactly equal to a boundary MUST remain in the less severe
  state because the thresholds are defined as “less than.”
- FR-5: Critical MUST take precedence when both warning and critical rules
  match.
- FR-6: Missing, invalid, unsupported, or expired readings MUST NOT be
  fabricated as normal or low-disk readings. Stale readings still within the
  existing evidence window MAY retain a warning only when labelled with their
  observation time.
- FR-7: Each affected node row MUST be visually distinct in both light and dark
  themes and MUST include an icon and explicit “Low disk” or “Critical disk”
  text; color alone is insufficient.
- FR-8: The affected row MUST show hostname/node identity and, for the selected
  offending filesystem, filesystem name, available capacity, usage percentage,
  and mountpoint. Total and used capacity MUST remain available in its details.
- FR-9: When several filesystems on one node are affected, the row MUST expose
  the worst severity and MUST allow the operator to see every affected
  filesystem and its concrete facts.
- FR-10: The Server Metrics accordion header MUST show the worst current disk
  severity and affected-node count while its body is collapsed.
- FR-11: The collapsed summary MUST remain present independently of the
  expanded body lifecycle and MUST NOT falsely show zero affected nodes while
  data is missing or loading.
- FR-12: Warning controls MUST be keyboard operable, screen-reader labelled,
  focus-visible, and free of nested conflicting interactions.

### Issue follow-through

- FR-13: Activating a warning MUST resolve a low-disk remediation issue for the
  node in the explicit project associated with the current issue/workspace
  context. When no explicit project is available, the warning remains visible
  but issue creation is disabled with explanatory text.
- FR-14: If an open low-disk issue already exists for that node in that project,
  the action MUST return and open it instead of creating another.
- FR-15: Concurrent activations and retries MUST preserve the same one-open-
  issue invariant durably; client-only button state or title matching is not
  sufficient.
- FR-16: Once the previous low-disk issue has `completed_at` set or is in the
  repository's established case-insensitive `Done`, `Cancelled`, or `Canceled`
  terminal status, a later observation MAY create a new issue for the same node.
- FR-17: A newly created node-level issue MUST include node ID, hostname, observation
  timestamp, filesystem, mountpoint, total, used, available, and usage
  percentage for every currently affected filesystem on the node, not only the
  row or filesystem that received the activation.
- FR-18: A newly created issue MUST request permanent remediation: identify
  consumers such as build caches, Nix store generations, old worktrees/
  workspaces, and logs; decide what can be garbage-collected on a schedule; and
  decide whether the volume should be resized.
- FR-19: The action result MUST distinguish a newly created issue from an
  existing issue and provide the issue identity needed for navigation.
- FR-20: While an action is pending, duplicate activation MUST be suppressed.
  On failure, the facts remain visible, the operator receives an actionable
  error, and retry remains possible.
- FR-21: The server MUST validate authorization, explicit project context, node
  identity, and submitted observation shape before resolving an issue.

### Configuration and boundaries

- FR-22: Warning free-percent, warning free-bytes, critical free-percent, and
  critical free-bytes thresholds MUST be configurable and their effective
  values MUST be supplied consistently to classification consumers.
- FR-23: Configuration MUST reject negative values and contradictory severity
  ordering, and the defaults MUST be documented for operators.
- FR-24: Alert evaluation and issue creation MUST NOT change node online state,
  drain state, lease state, health, affinity, or scheduler eligibility.
- FR-25: The feature MUST use the existing filesystem metrics facts and MUST
  NOT add an independent disk sampler.
- FR-26: Failure or malformed data for one node MUST NOT blank warnings or
  metrics for other nodes.
- FR-27: Changes MUST remain limited to the Vibe Kanban service and its
  governing deployment configuration.

## Out of Scope

- Automatically deleting data, running garbage collection, or resizing disks.
- Blocking or changing scheduling based on disk alerts.
- Email, chat, pager, or operating-system notifications.
- Creating issues without an explicit project/workspace context.
- Changes to any hosted service other than Vibe Kanban.

## Acceptance Criteria

- [ ] A filesystem below either default warning boundary visibly highlights its
  node with warning icon, text, and concrete filesystem/Avail/Use%/mount facts.
- [ ] A filesystem below either critical boundary receives critical treatment,
  including when only its byte or only its percentage rule matches.
- [ ] Exact boundary values and absent/invalid data do not create false alerts.
- [ ] The collapsed accordion header shows the worst severity and affected-node
  count without keeping the expanded metrics body mounted.
- [ ] Warning interaction is usable by mouse, keyboard, and screen reader.
- [ ] First activation creates a pre-filled permanent-remediation issue and
  opens it; a repeat or concurrent activation opens the existing issue.
- [ ] Closing the existing incident permits a future activation to create a new
  incident.
- [ ] Failed issue resolution is reported and can be retried without losing the
  displayed metrics.
- [ ] Default and overridden thresholds are observable, documented, and covered
  by validation tests.
- [ ] Existing metrics rendering, failure isolation, streaming lifecycle, and
  scheduling behavior remain unchanged.

## Open Questions

None. `/speckit.clarify` resolved the initial project-context, identity-scope,
and multi-filesystem questions.
