# Scheduled wakeups remain pending

Investigate overdue durable wakeups in workspace 84ce5feb-390e-4879-8aa2-c07d36e933b6 and correct the confirmed cause within Vibe Kanban only.

## Required behavior
Due wakeups must be reconsidered after restart and when their session becomes eligible. Active turns must retain serialization; cancelled or delivered wakeups must not run again. Persistent failure must be distinguishable from normal scheduling delay. Preserve existing durable state and avoid manually replaying user prompts during diagnosis.

## Investigation and acceptance
Read affected wakeup/session state, trace scheduling and delivery ownership, and reproduce the confirmed failure with a focused regression test. Implement the smallest safe correction, verify existing delivery safeguards, document the causal evidence and operational limitations, obtain independent Codex review, record reusable project knowledge, and open and merge a PR against the repository base branch.

Root cause is not yet established. Deployment configuration changes, if necessary, are limited to the Vibe Kanban service in homelab/modules/vibe-kanban-rebuild.nix.

## Confirmed cause and repair
Production standalone startup in crates/server/src/main.rs bypasses the shared
startup::initialize_deployment function, omitting the durable delivery dispatcher
and startup reconciliation marker. Delegate to the shared initializer; preserve
listener setup and shutdown behavior. See ../PRIOR_KNOWLEDGE.md and the SpecKit
artifacts in ../homelab/specs/vk/d5c5-scheduled-wakeup/.
