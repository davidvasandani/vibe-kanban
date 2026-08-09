# Implementation Plan: Mobile Deploy Status

1. **Establish the metadata contract**
   - Trace the existing `/api/info` `UserSystemInfo` response, Rust build stamping, `local-build.sh` release publication, and generated TypeScript registration.
   - Add an optional ISO-8601 deployment/build timestamp beside `version`, preserving compatibility for unstamped development builds.
   - Reuse one timestamp value for both the compiled binary and `release.json` so the UI and deployed release manifest describe the same build.

2. **Generate and consume the API type**
   - Update the Rust `UserSystemInfo` source type and response construction.
   - Regenerate `shared/types.ts` through the repository generator; do not edit it manually.
   - Extend `useUserSystem`/its controller to expose the optional timestamp alongside `appVersion`.

3. **Build reusable deploy-status presentation**
   - Add a small pure elapsed-time formatter with deterministic boundary behavior and focused unit tests.
   - Add a compact deploy-status UI element that renders `SHA · age`, links production SHAs to GitHub, renders `dev` without a link, and supplies a descriptive accessible label/title.
   - Refresh the elapsed label on a bounded timer matching the formatter's precision.

4. **Wire the mobile header**
   - Pass deployment metadata from `SharedAppLayout` into the mobile `NavbarContainer`, then into `Navbar`.
   - Place the status in the mobile top row while preserving existing sync, navigation, settings, command-bar, and user actions.
   - Add shrink/truncation/responsive classes so phone widths do not overflow.

5. **Verify behavior**
   - Add or update component tests for a production SHA, missing timestamp, and `dev` behavior.
   - Run locked dependency setup if needed, generated-type checks, targeted frontend/backend tests, TypeScript checks, formatting, and lint proportional to the touched files.
   - Inspect the mobile header at a representative phone viewport if a local preview is practical.

6. **Review and finish**
   - Execute the SpecKit task list in dependency order.
   - Run independent Codex diff review, address confirmed findings, and repeat until no significant findings remain.
   - Distill reusable deployment-metadata/mobile-header knowledge into the project knowledge base and update its index.
   - Commit the knowledge-base update, then merge the task branch into its configured base branch.
