# Preview app URL persistence

The preview browser has two distinct URL authorities that must not be conflated:

- the detected or manually overridden **origin**, which decides which development
  server Vibe Kanban loads; and
- the latest auto-detected **route** (`pathname + search + hash`), which represents
  the user's current location inside that application.

Persisting a complete navigated URL as the manual override pins an old port and
disables normal server detection. Store the route separately in the
workspace-scoped `PREVIEW_SETTINGS` scratch payload, then rebase it onto the
freshly detected origin when a preview iframe is initialized or intentionally
remounted. Do not feed each persistence echo back into the live iframe `src`:
that turns ordinary SPA navigation into a destructive reload.

The injected preview bridge reports complete `location.href` values and already
orders events by document/sequence/timestamp. Before persistence or display,
remove Vibe Kanban's transport parameters (`_refresh`, `_vk_workspace`,
`_vk_execution`, `_vk_generation`) while preserving the application path, query,
and fragment. Hash fragments require special care: they never reach the proxy
server, but must remain on the browser-side iframe URL for hash-routed apps.

Scratch updates replace the complete payload, so independently debounced URL,
route, and viewport writes can erase each other if they merge against a lagging
WebSocket snapshot. Serialize local writes against the latest intended complete
settings and reconcile stream echoes by the `updated_at` version returned from
the update response. A navigation event should be persisted once; a later
cross-tab scratch echo is not a new local navigation event.

Finally, bind navigation state to the workspace and wait for that workspace's
scratch snapshot before loading the iframe. `PreviewBrowserContainer` is reused
across workspace selections, and loading the detected root before settings arrive
can overwrite the route that was about to be restored.

## Contributed by

- vk/4aeb-app-urls-for-mad
