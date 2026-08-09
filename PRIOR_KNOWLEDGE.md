# Prior Knowledge: Mobile Deploy Status (`7596-deploy-status-mo`)

The populated project knowledge bases (`docs/knowledge-base/` and `wiki/`) were searched read-only for deployment metadata, the app rail/navbar, and mobile responsive UI.

## Relevant pages

| Page | Reusable guidance |
| --- | --- |
| `wiki/self-hosted-deployment.md` | The self-hosted release already writes `release.json` containing `sha`, `build_id`, and `built_at`; binary and static releases share one build id, and deployment is an atomic `current` symlink flip. This is the most authoritative existing definition of “deployed at.” |
| `wiki/appbar-rail-and-org-tiles.md` | Desktop utility ordering deliberately ends with version information. The mobile equivalent is `Navbar`, not the desktop `AppBar`; retain existing command/settings/user entry points. |
| `wiki/workspace-context-bar-responsive-visibility.md` | Responsive mobile layout and physical-device detection are different signals. Header presentation should follow the existing responsive `mobileMode` path rather than introducing device detection. |
| `wiki/mobile-kanban-scrolling.md` | Phone-width behavior must be verified explicitly; narrow mobile containers require deliberate overflow constraints. Component tests should run through package scripts because the ambient development environment may set production React behavior. |
| `wiki/workspace-navbar-breadcrumbs.md` | Keep async data/state mapping outside complex navbar markup where possible and test pure presentation/state helpers separately. |
| `docs/knowledge-base/worktree-formatting-prerequisites.md` | In a fresh worktree, install with the frozen lockfile before frontend verification and run repository format/check commands through their package scripts. |

## Existing code facts confirmed during recall

- `crates/server/build.rs` already embeds a short Git SHA.
- The server config/info response exposes that value as `version`, which reaches `useUserSystem().appVersion`.
- `SharedAppLayout` already owns both `appVersion` and deploy-update polling, and renders both desktop `AppBar` and mobile `Navbar`; it is the natural metadata handoff point.
- Desktop `AppBar` already links non-`dev` revisions to the exact GitHub commit and treats `dev` as non-linking text.
- Mobile `Navbar` currently has no deploy metadata props or indicator.

## Planning implications

1. Prefer the existing self-hosted `release.json.built_at` as deployment-time truth if it can be surfaced cleanly at runtime; do not label a compiler wall-clock timestamp as deploy time without qualification.
2. If the runtime cannot access release metadata in all supported layouts, define a safe fallback and preserve `dev` behavior.
3. Add metadata at the `SharedAppLayout` → `Navbar` boundary; do not create a second config fetch in the presentation component.
4. Reuse desktop commit-link semantics and keep the mobile indicator compact enough that command, settings, notifications, and user controls remain usable.
5. Test elapsed-time formatting as a pure helper and the mobile navbar rendering at the component boundary.
