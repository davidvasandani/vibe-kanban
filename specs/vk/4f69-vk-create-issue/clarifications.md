# Clarifications: Scrollable Create-Issue Settings

`/speckit.clarify` found no remaining blocking questions after comparing the
report, screenshot, `SPEC.md`, `PRIOR_KNOWLEDGE.md`, and current panel layout.

## Resolved decisions

| Question | Decision | Evidence |
| --- | --- | --- |
| Should the Create Issue action scroll with the form or become a sticky footer? | Keep it in the existing scrolling form; do not introduce a sticky footer. | The request is that cut-off settings "can't be scrolled," not that controls need rearrangement. The button and create-only settings already share one content region, preserving their order is the smallest behavior-preserving fix, and the screenshot shows the existing action at the content bottom. |
| Is the correction limited to the shared issue panel, or should application-wide mobile viewport handling change? | Limit it to the shared issue panel's flex/scroll sizing contract. | Both mobile and desktop hosts already provide `h-full` and `overflow-hidden`. The panel body is already designated `flex-1 overflow-y-auto`; its missing shrink allowance is the local mismatch. Global viewport/navigation changes would broaden the blast radius without evidence. |
| Does this apply only to create mode? | Correct the shared body for create and edit modes. | Both modes use the same panel shell/body. A common layout correction avoids mode-specific divergence and satisfies constitution IV's shared-component boundary. |
| What regression evidence is appropriate? | A rendered-DOM component test must assert the shell/body class contract and verify the create controls are descendants of the body. | JSDOM does not perform layout or prove actual pixel scrolling, but the repository already has a `KanbanIssuePanel` rendered-component suite. Testing the explicit flex/overflow contract is deterministic and local to the regression. |

## Remaining open questions

None.
