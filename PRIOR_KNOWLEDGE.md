# Prior Knowledge: Desktop Deploy Status (`VAS-377`)

The populated project knowledge bases (`docs/knowledge-base/` and `wiki/`)
were searched read-only for deploy identity, right-drawer composition, flex
layout, and verification guidance.

## Relevant pages

| Page | Reusable guidance |
| --- | --- |
| `wiki/self-hosted-deployment.md` | Deployment identity is the immutable artifact's embedded Git SHA plus optional build/publish timestamp from `/api/info`; service restarts are not deployments, missing timestamps must not fabricate age, and relative age can update locally. |
| `wiki/flexible-collapsible-panel-stacks.md` | `RightSidebar.tsx` is the desktop workspace right drawer. Its bounded flex stack gives expanded collapsible sections the remaining height, while non-collapsible intrinsic rows must remain `flex-none h-auto`. The complete `min-h-0` chain and outer overflow fallback must be preserved. |
| `docs/knowledge-base/nested-flex-scroll-containment.md` | Fixed content above a scrolling flex body must be `shrink-0`, with scroll ownership and `min-h-0` applied deliberately. Rendered-DOM tests can protect class contracts even though JSDOM cannot calculate layout. |
| `docs/knowledge-base/worktree-formatting-prerequisites.md` | Run `pnpm install --frozen-lockfile` before repository formatting in a fresh worktree; the preformat hook intentionally fails before partial mutation if package-local formatter shims are missing. |

## Distilled constraints for the spec and plan

1. Reuse `useUserSystem` and the existing shared `DeployStatus`; do not create a
   second API request, runtime release-file read, timestamp meaning, or desktop
   formatter.
2. Insert the desktop status at the `RightSidebar` composition boundary, before
   the section list. This is the drawer controlled by `ToggleRightSidebar`.
3. “No toggle / always visible” is best represented by a fixed intrinsic row,
   not by a new `CollapsibleSectionHeader` or persisted expansion key.
4. Preserve the drawer's bounded flex and overflow behavior. The fixed row must
   not claim a share of the height intended for expanded content sections.
5. Preserve compatibility: a real SHA links to its commit; `dev` is non-linking;
   absent or invalid timestamps leave the revision visible without an age.
6. Validate the rendered structure/classes and shared status behavior with
   focused tests, then run repository-owned type, generated-contract, lint,
   format, and relevant test commands after locked dependency setup.

## Scope conclusion

No homelab or other-service change is required. The project/issue contextual
panel is distinct from the persistent workspace right drawer and is not part of
this task.
