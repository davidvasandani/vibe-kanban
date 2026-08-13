# Contract: SpecKit Artifact Ownership

- Durable path: `specs/vk/<current-task-id>/`.
- The task ID comes from current task/branch context, not a previous hard-coded
  feature path.
- A specification records its owner task ID.
- Refresh is allowed when requested owner equals recorded owner.
- A different recorded owner aborts before the first file write.
- History restoration copies exact source versions before any intentional
  reference repair.
- Pipeline commands and generated artifacts name the same task-owned path.
