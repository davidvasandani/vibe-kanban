# Personal Repository Commit Link

## Problem

The application version shown in the app bar links commit hashes to the upstream
`BloopAI/vibe-kanban` repository. This checkout is distributed from the personal
`davidvasandani/vibe-kanban` repository, so the link can send users to a commit
page in the wrong repository.

## Requirements

- For a non-development application version, the app-bar version link must point
  to `https://github.com/davidvasandani/vibe-kanban/commit/<version>`.
- The displayed version text, tooltip, new-tab behavior, and security-related
  link attributes must remain unchanged.
- The special `dev` version must remain plain text and must not become a link.
- Other upstream project links, such as documentation, releases, and community
  links, are outside the scope of this change.

## Verification

- Confirm the app-bar source builds a commit URL using the personal repository.
- Run the relevant frontend formatting and type/lint checks for the changed
  package, or the closest repository-provided checks available.
