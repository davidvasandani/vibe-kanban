# Feature Specification: Remember Filter Settings

Feature dir: `specs/vk/e831-remember-filter/`
Status: Clarified

## Summary
Keep board choices on return visits and let people deliberately save reusable defaults from the Filters dialog.

## User stories
- As a user, I want my filters and sorting restored after navigation/reload.
- As a user, I want a personal default without changing another user's choices.
- As a user, I want an explicit site-wide save action rather than accidentally sharing every edit.

## Functional requirements
- FR-1: Remember priority, assignee, tags, sorting direction/field, sub-issue, workspace and blocked toggles per project/view/user.
- FR-2: Offer explicit save personal default and save site default actions in the Filters view.
- FR-3: Resolve remembered view choices > personal default > site default > built-in Team/Personal defaults.
- FR-4: Default saves exclude search text. Tags remain scoped to their source project. Me remains relative to the viewing user.
- FR-5: Restore defaults without overwriting another user's settings; allow clearing saved defaults.
- FR-6: Show persistence failures and do not report success before save completes. Do not overwrite edits with late initialization.

## Acceptance criteria
- Reload and navigate away/back after changing each filter; values are retained for that user and view.
- A second user cannot inherit the first user's remembered settings/personal default.
- Site defaults apply when no personal or remembered override exists.
- Empty arrays and false values override defaults correctly.
- A tag default from one project cannot hide everything in a different project.
- Both frontend entrypoints typecheck; unavailable storage surfaces errors.

## Out of scope
Other services, changing filter matching semantics, and changing unrelated UI preferences.

## Open questions
No blocking questions. Default assumption: site-wide means everyone on this self-hosted Vibe Kanban instance, across projects. Defaults are separate for Team and Personal views. Remembered choices persist in this browser under the current user, project and view; explicit personal/site defaults persist on the server. The cloud-only frontend has no shared scratch backend and must not advertise a browser-only setting as site-wide. Signed-out users can remember browser choices but cannot save personal defaults. Existing shared UI settings must not be migrated into a personal default because their author is unknown.
