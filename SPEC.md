# Debug VK Load: execution admission under I/O pressure

## Problem
An execution failed with worker HTTP 503: I/O PSI full avg60 41.6% exceeded the configured 40% admission threshold. Identify why that worker was selected, whether a healthy alternative was available, and whether pressure remains elevated. Distinguish observed runtime evidence from inference.

## Scope and requirements
Only Vibe Kanban source and its hosting configuration in homelab/modules/vibe-kanban-rebuild.nix are in scope. Preserve pressure protection, execution ownership and idempotency. Do not weaken thresholds merely to suppress errors. Improve the demonstrated failure path with a bounded change supported by diagnostics and regression coverage. If no eligible worker exists, report an actionable failure without duplicate execution or unbounded retries.

## Acceptance
Document the causal path, validate selection/admission behavior under pressure and normal operation, and retain existing pinning/locality guarantees. Record unavailable live evidence explicitly. Follow the ordered WikiLLM + SpecKit pipeline, independent Codex review, knowledge capture and PR merge after verification.
