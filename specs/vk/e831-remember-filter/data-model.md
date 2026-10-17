# Data model and contracts

Remembered browser entry: version 1, full KanbanProjectViewPreferences, keyed by user (anonymous sentinel when signed out), project, Team/Personal view. Search is stripped before persistence. Active view selection is also keyed by user/project.

Default entry: {projectId, preferences}, per Team/Personal view in kanban_project_view_preferences of an isolated UI_PREFERENCES scratch. Site UUID: 00000000-0000-0000-0000-000000000002. User UUID: authenticated user ID. Clearing removes only that view default. Decode and validate stored JSON before use. Invalid data falls back safely.

Resolution: remembered > personal > site > built-in. Copy snapshots before persistence. Cross-project default application drops tag IDs. Default action promises resolve after scratch update; failure is displayed in dialog and does not claim success.
