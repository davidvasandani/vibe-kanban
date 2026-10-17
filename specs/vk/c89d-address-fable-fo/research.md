# Research Notes

## Existing URL authority

`devtools_script.js` already reports `location.href`, listens for `hashchange`,
and detects URL changes on an interval. `usePreviewNavigation` already rejects
stale per-document sequence numbers and timestamps. The missing behavior is not
event detection; it is durable synchronization after the event reaches the
container.

## Existing persistence authority

`usePreviewSettings` stores preview state in a workspace-scoped scratch payload.
Its `url` field currently means a user override and therefore cannot safely be
reused for ordinary navigation: doing so would pin the previous server's origin
and disable normal auto-detection semantics. A separate optional route field is
the smallest compatible representation.

## Fragment handling

`transformProxyUrlToDevUrl` and same-port URL submission already preserve hash
fragments, but initial proxy iframe construction currently uses only pathname and
query. Adding the fragment to the browser-side iframe URL is required for
hash-routed selections and does not alter the HTTP request sent to the proxy.

## Alternatives rejected

- Persist the full navigated URL in `url`: rejected because it silently converts
  auto-detected navigation into a manual override and pins a possibly stale port.
- Keep the latest route only in React/local storage: rejected because existing
  preview settings already provide durable, workspace-scoped server persistence.
- Persist full browser history: rejected as unnecessary for restoring the last
  selected worksheet.

## Dependencies

No new library or service dependency is needed.
