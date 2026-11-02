# /speckit.tasks: README product guide

Plan: ./plan.md. Paths relative to repository root.

## Layer 1: evidence
- [x] T001 [P] Audit shipped feature families, historical additions and source evidence; write specs/vk/f960-update-readme/research.md.
- [x] T002 [P] Install locked dependencies and record tool/setup availability in specs/vk/f960-update-readme/validation.md.

## Layer 2: README, depends on T001
- [x] T003 Rewrite README.md with product-first coverage, verified setup, component boundaries and ancestry footnote.

## Layer 3: validation, depends on T002/T003
- [x] T004 Run formatting and link/command/claim checks; record results in specs/vk/f960-update-readme/validation.md.
- [x] T005 Run independent Codex CLI review; resolve findings and record outcome in specs/vk/f960-update-readme/review.md.

## Layer 4: knowledge, depends on T005
- [x] T006 Update wiki/readme-product-guide.md and wiki/INDEX.md with reusable findings; commit the knowledge base and task documentation.

## Layer 5: delivery, depends on T006
- [ ] T007 Push branch, create PR against main, inspect checks and merge; record delivery in specs/vk/f960-update-readme/validation.md before final commit where practical.

[P] tasks use independent outputs and may run together. Later layers are sequential. No agent delegation is needed for the audit; independent review uses Codex CLI as requested.
