# Prior knowledge: AWS SSO and agent CLI visibility
Task: vk/c817-aws-sso-sign-in

Searched the local project knowledge base (`vibe-kanban/wiki`) and homelab deployment knowledge (`homelab/docs/knowledge`) read-only for AWS, CLI tools, sandbox, and isolated homes.

- `vibe-kanban/wiki/managed-cli-tool-catalog.md`: CLI installation is machine-scoped. `utils::shell::append_cli_tools_to_path` appends the spawning host’s app-data bin directory. Worker execution and interactive terminals each need augmentation. Never forward coordinator absolute tool paths to workers without a deployment contract.
- `vibe-kanban/wiki/codex-credential-refresh.md`: worker `prepare_scoped_home` shares runtime credential assets by symlink while snapshotting per-execution configuration. Credentials need live state, not stale turn snapshots; concurrency and refresh matter.
- `vibe-kanban/wiki/self-hosted-deployment.md`: source and deployment are separate repos. Immutable releases, health-gated restart, and worker rollout must be considered; source merge is not proof of running behavior.

Implications: follow worker-local environment construction, distinguish coordinator and worker identity in reporting, and test late login/token refresh against isolated homes. No existing AWS-specific knowledge page was found.
