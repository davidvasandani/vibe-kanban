# Clarifications: Timestamp the Workspace Chat Log

No user-blocking questions remain.

1. A compact label shows local time for entries on the user's current local
   calendar day. Entries from another day show a short local date and time. The
   underlying semantic `<time>` value and accessible/title text retain the full
   timestamp in all cases.
2. A grouped row represents activity completed through its latest member, so it
   shows the most recent valid member timestamp. Expansion continues to expose
   each member and its own timestamp.
3. “Every message, event, action” means meaningful persisted conversation
   entries and their grouped projections. Synthetic loading, next-action, token
   accounting, and other non-log presentation controls remain untimestamped.
4. Invalid or absent timestamps stay absent. Falling back to the browser clock
   would falsely date historical replay as current activity.
