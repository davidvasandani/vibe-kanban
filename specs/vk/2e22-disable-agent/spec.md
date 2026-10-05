# Feature Specification: Disable unused agents

**Feature dir**: `specs/vk/2e22-disable-agent/`
**Status**: Draft

## Summary
Settings → Agents lists every built-in coding agent: Claude Code, Codex,
Gemini, Amp, Opencode, Cursor, Qwen Code, Copilot, Droid and Grok. Every
agent picker in the app offers all of them too. Most hosts use one or two,
so the rest are noise in every picker, and built-in agents cannot be deleted.
This feature lets the user turn off agents they don't use. A disabled agent
disappears from agent pickers. It stays in Settings, keeps its
configuration, and can be turned back on at any time.

## User Stories
- As a user who only runs Claude Code and Codex, I want to turn off the other
  agents so that pickers show only the agents I use.
- As a user, I want a disabled agent's configurations kept, so that turning
  it back on restores exactly what I had.
- As a user, I want to see at a glance in Settings which agents are disabled.
- As a user, I don't want to break my setup by accident, so I can't disable
  the agent new work uses by default, and I can't disable every agent.
- As a user with an existing session or workspace on an agent I later
  disable, I want that work to keep running and to keep showing its agent.

## Functional Requirements
- FR-1: Settings → Agents provides a per-agent control to show or hide the
  agent in agent pickers. The change follows the page's existing
  unsaved-changes flow: Save applies it, Discard reverts it.
- FR-2: The Agents list marks disabled agents visibly with a label and a
  muted style, without reordering the list.
- FR-3: A disabled agent remains selectable in Settings. Its configurations
  can still be viewed, edited, created and deleted.
- FR-4: After a save, a disabled agent does not appear as a choice in any
  agent picker:
  - the new-workspace agent picker
  - the new-session agent picker
  - the General default-agent dropdown
  - the auto error remediation agent selector
  - the Start review and Resolve conflicts agent selectors
  - the onboarding agent choice
- FR-5: A picker whose current value is a disabled agent still shows that
  agent, so the displayed selection always appears in its own menu.
- FR-6: When a picker falls back automatically (last used agent, then the
  default agent), it skips disabled agents and uses the first enabled agent
  instead. An agent the user explicitly picked in that picker, or a saved
  draft, is respected.
- FR-7: The default agent cannot be disabled. The control is locked and
  explains that another agent must be made the default first.
- FR-8: The last enabled agent cannot be disabled. The control is locked and
  explains why.
- FR-9: A disabled agent cannot be made the default.
- FR-10: The setting is stored per host, with that host's agent
  configuration, and persists across restarts. Existing configuration files
  load unchanged. Saving without changing the setting adds nothing to the
  file.
- FR-11: Re-enabling an agent restores it to every picker, and its
  configurations are intact.
- FR-12: Automatic agent recommendation (onboarding) never recommends a
  disabled agent.
- FR-13: Disabling does not stop anything that already uses the agent:
  running or existing sessions, follow-ups, scheduled pipelines, auto
  remediation already set to that agent, and API/MCP requests that name it
  all keep working.

## Out of Scope
- Blocking execution of a disabled agent (this is not access control).
- Disabling individual configurations (variants) of an agent.
- Reordering or sorting agents by enabled state.
- Syncing the setting across hosts.

## Acceptance Criteria
- [ ] Unchecking Qwen Code's control and saving hides Qwen Code from all
      pickers listed in FR-4. Settings marks it Disabled.
- [ ] After a reload or restart, Qwen Code is still disabled.
- [ ] Re-enabling and saving restores Qwen Code everywhere. Its custom
      configuration is unchanged.
- [ ] The default agent's control is locked with an explanation, and so is
      the last enabled agent's.
- [ ] A disabled agent's configurations offer no "Make Default" action.
- [ ] A new-session picker whose last-used agent is now disabled starts on an
      enabled agent.
- [ ] An existing workspace on a disabled agent continues to accept
      follow-ups.
- [ ] A configuration file without the setting loads with every agent
      enabled, and saving it unchanged writes no new key.
- [ ] Automated tests cover storage round-trip and persistence, the picker
      filtering rule (including keeping the current value), and the lock
      rules.

## Clarifications

Resolved during `/speckit.clarify` (2026-10-05). No answers came with the
command, so each question takes the narrowest choice that the request
("disable unused agents", shown on the Settings → Agents screen) and the
constitution support.

- **Q: Per host or global?** → **Per host.** Settings → Agents edits one
  host's agent configuration (it can target a remote machine). Whether an
  agent is installed and used is a per-host fact. Constitution L puts per-host
  agent preferences next to the configuration they qualify. FR-10 already
  says this.
- **Q: Block execution, or only hide?** → **Only hide.** The request is about
  clutter. Blocking would break existing workspaces, pipelines, remediation
  settings and MCP automation that name the agent, and constitution L says a
  picker preference is a display filter, not access control. FR-13 and Out
  of Scope already say this.
- **Q: Sort disabled agents to the bottom?** → **No.** The list keeps its
  order, so the agent the user is toggling never jumps away from under the
  pointer. The muted style and label (FR-2) are enough to tell disabled
  agents apart.

## Open Questions
None.
