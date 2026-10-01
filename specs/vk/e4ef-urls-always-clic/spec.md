# Feature Specification: URLs always clickable

**Feature dir**: `specs/vk/e4ef-urls-always-clic/`
**Status**: Draft

## Summary
Agents often report results with bare URLs, such as "pushed to PR #1803:
https://github.com/…/pull/1803." Chat messages, issue descriptions and
comments show these URLs as plain text, so the user has to select and copy
them. Links to `http://` destinations, such as local dev servers and LAN
addresses, are shown but disabled even when written as markdown links. This
feature makes every web URL in rendered, read-only markdown a working link.

## User Stories
- As a user reading an agent reply, I want a bare PR or dashboard URL to be a
  link so that I can open it in one click.
- As a user, I want a URL at the end of a sentence or inside parentheses to
  link to the right address, without the trailing period or bracket.
- As a user, I want `http://localhost:…` and LAN links to open, because my
  homelab services do not all use TLS.
- As a user composing a message, I want my text left as I typed it, with no
  surprise link conversion while I write.

## Functional Requirements
- FR-1: In read-only rendered markdown, every bare `http://` or `https://`
  URL in prose is shown as a clickable link. That includes paragraphs, lists,
  headings, quotes, tables and emphasised text.
- FR-2: Clicking such a link opens the URL in a new tab or window. The opened
  page gets no reference back to the app.
- FR-3: Trailing sentence punctuation (`. , ; : ! ?` and quotes) is not part
  of the link. A trailing closing bracket is excluded unless the URL itself
  contains the matching opening bracket.
- FR-4: A URL is detected even when it directly follows an opening bracket,
  a quote or `<`.
- FR-5: Several URLs in one line are each linked separately.
- FR-6: Explicit markdown links whose destination is `http://` are clickable,
  the same as `https://` ones.
- FR-7: Nothing is linked inside fenced or indented code blocks. A URL in
  inline code becomes a link and keeps its code styling. Text that is already
  a link keeps its existing destination.
- FR-8: Only `http` and `https` are made clickable. Other schemes, such as
  `javascript:`, `data:` and `file:`, stay non-clickable.
- FR-9: While composing (editable mode), text is not converted. The stored
  markdown of any message or description does not change because of this
  feature.
- FR-10: The existing in-app issue links keep their current behaviour: they
  are clickable and open in the same window.

## Out of Scope
- URLs without a scheme (`www.example.com`, `example.com/path`) and email
  addresses.
- Surfaces that already linkify with their own logic: raw process logs and the
  simple summary markdown.
- In-app client-side navigation for links that point into the app.

## Acceptance Criteria
- [ ] The screenshot sentence "…description: https://github.com/sweetgreen/terraform-infrastructure/pull/1803. Nothing…"
      renders an anchor to `…/pull/1803` that opens in a new tab, with the
      period outside the anchor.
- [ ] `(see https://example.com/a_(b))` links `https://example.com/a_(b)`.
      `(https://example.com)` links `https://example.com`.
- [ ] Two URLs on one line render two anchors.
- [ ] A fenced block containing a URL renders no anchor. Inline
      `` `https://example.com` `` renders an anchor around the code-styled text.
- [ ] `[docs](http://localhost:3000)` and a bare `http://localhost:3000` are
      both clickable. `[x](javascript:alert(1))` is not.
- [ ] Exporting the read-only document back to markdown gives the original
      bare URL text.
- [ ] The existing issue-link tests still pass.

## Clarifications
- Q: Should plain `http://` be clickable, not only `https://`, given that
  upstream deliberately limited links to `https://`? A: Yes. The request is
  that URLs are "always" clickable, and this deployment links to plain-HTTP
  homelab and dev-server addresses. The risk that matters is the scheme, not
  TLS: `javascript:` and `data:` stay blocked by the allow-list (FR-8), and
  every external link opens with `noopener noreferrer`.
- Q: Should URLs inside inline code be clickable? A: Yes, because "always"
  covers them. The link wraps the code-styled text. A URL never matches a
  diff file path, so it cannot collide with the file-path click behaviour on
  inline code. Fenced code blocks stay literal: they are multi-line program
  text, where linking would interfere with selecting and copying.

## Open Questions
None.
