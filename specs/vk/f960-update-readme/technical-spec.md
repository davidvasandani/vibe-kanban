# Technical spec: README product overview

Task: f960-update-readme. Repository: davidvasandani/vibe-kanban.

## Objective
Replace the inherited fork-first README with an accurate product guide covering both established functionality and subsequent additions. Keep upstream ancestry in a footnote. Base claims on current source, checked-in documentation, project knowledge and deployment configuration.

## Scope
Documentation only in the Vibe Kanban repository: README, reusable wiki knowledge and required pipeline artifacts. Read homelab configuration only to establish deployment facts; do not change other services or deployment behavior.

## Requirements
- Explain issues, projects, agent workspaces, supported executors, review, previews, Git integration, automation, MCP, integrations, search and self-hosting.
- Distinguish application capabilities from optional infrastructure and configured integrations.
- Document reproducible source installation/development commands and current deployment ownership.
- Link to focused repository docs; remove obsolete personal commentary, unrelated support directions and unsupported claims.
- Preserve license attribution in a footnote; do not present upstream npm packages as this repository's distribution.

## Acceptance and verification
Trace feature claims to source/docs, verify local links and commands against scripts/manifests, run required dependency setup and formatting, inspect diff for scope. Complete independent Codex review and resolve significant findings. Record reusable findings in the wiki, commit, open a PR against the repository base branch and merge after checks permit it.

## Risks
Existing documentation may lag code. Historical feature lists are discovery aids, not proof. Cluster operation, remote collaboration and single-user mode have distinct prerequisites. Avoid secrets and environment-specific claims without evidence.
