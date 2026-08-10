# Clarifications: Mobile Deploy Status

## Resolved decisions

| Question | Decision | Rationale |
| --- | --- | --- |
| Where is status shown? | Always visible in the mobile top header, as a compact status item in the right-side utility cluster. | The request explicitly asks to add the missing information to the header. Hiding it in a menu would not restore desktop/mobile information parity. |
| How is age written? | Use a compact single-unit label: `now` below one minute, then whole `m`, `h`, or `d` units (for example `7m`, `3h`, `12d`). | The existing header has severe horizontal constraints. These units match relative-time vocabulary already used elsewhere in the UI and remain scannable beside a SHA. |
| Does the SHA link anywhere? | The SHA is a link to the deployed commit for non-`dev` builds, matching desktop behavior. Development metadata is plain text. | Reusing desktop behavior makes the same operational identity equally actionable on mobile without adding a new action. |
| What timestamp defines deployment? | Server process start time, serialized by the existing system-information response. | A browser timestamp resets on reload and commit time can predate deployment; neither answers how long the running deployment has been active. |
| What happens with partial metadata? | Render the available values only; omit the status item if neither value is usable. | System metadata is diagnostic and must fail soft. A mixed-version or development response must not break mobile navigation. |
| How often does age update? | Recompute once per minute while mounted. | Minute precision is the finest displayed after `now`; a one-minute timer is bounded and sufficient. |

## Remaining open questions

None.
