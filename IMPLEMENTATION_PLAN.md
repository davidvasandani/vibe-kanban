# Implementation Plan

1. Locate the app-bar application-version link and confirm its existing behavior
   for development and commit-hash versions.
2. Change only the commit-link repository owner from `BloopAI` to
   `davidvasandani`, preserving the hash interpolation and link behavior.
3. Run repository formatting and focused frontend validation.
4. Review the complete diff with an independent Codex review, address any
   significant findings, and repeat validation and review as needed.
