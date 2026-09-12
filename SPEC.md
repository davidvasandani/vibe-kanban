# Technical Specification: Preserve Preview App Navigation URLs

## Problem

The Vibe Kanban preview browser detects and loads the Mad Minutes development
server, but an in-app worksheet selection is not retained as the workspace's
preview URL. Returning to or reloading the preview can therefore reopen the
application at its detected/default URL instead of the selected worksheet URL.

## Scope

This change is limited to the Vibe Kanban service. It will update preview-browser
URL state handling in `packages/web-core` and, if required by the established
navigation protocol, the Vibe Kanban preview proxy. It will not modify Mad
Minutes or any other service.

## Desired behavior

- When an embedded preview navigates, Vibe Kanban records the latest user-visible
  application URL for that workspace.
- Path, query string, and fragment are preserved, because any of them may encode
  the selected worksheet.
- Reopening or remounting the preview uses the last recorded application URL
  rather than falling back to the development server's root URL.
- Internal Vibe Kanban transport parameters such as `_refresh` are not persisted
  as application state.
- Existing manual URL overrides, proxy-to-development URL translation, remote
  previews, and navigation controls continue to behave as before.

## Technical approach

Treat navigation reported by the injected preview bridge as the authoritative
current page once it is accepted by `usePreviewNavigation`. Synchronize that
canonical, transport-clean URL into the workspace's existing preview settings
storage without turning ordinary navigation into a permanent manual override
that defeats future development-server detection. Restore the stored current URL
when the same workspace preview is mounted again. Keep persistence isolated per
workspace and avoid write loops caused by scratch-data refreshes.

The exact storage representation and synchronization boundary will be finalized
after the required knowledge-base and SpecKit analysis.

## Acceptance criteria

1. Select a worksheet in Mad Minutes so its route differs from the app root,
   leave/remount the Vibe Kanban preview, and observe the selected worksheet
   route reload.
2. The preserved URL includes pathname, query, and hash components.
3. `_refresh` and other preview-only routing metadata do not leak into the saved
   application URL shown to the user.
4. Navigation state from one workspace is never restored in another workspace.
5. Automated tests cover persistence/restoration and guard against persistence
   loops or stale navigation overwriting a newer selection.

## Verification

- Run focused frontend unit tests for preview URL/navigation/settings behavior.
- Run frontend type checking and formatting/lint checks required by the repo.
- Exercise the preview manually when a suitable local fixture is available.

## Risks

- Persisting every bridge event could create excessive scratch writes.
- Conflating a navigated URL with an explicit override could pin a stale port or
  prevent normal auto-detection after a server restart.
- Proxy URLs must be normalized before storage so host-scoped and local preview
  modes restore correctly.
