# Technical plan: Button prompts
Spec: ./spec.md. Task: vk/bab2-button-prompts.
React/TypeScript shared UI serves local and remote frontends.
1. SessionChatBox.tsx: optional issue display ID and optional explicit message argument to onSend. Render three buttons only where empty existing-session Send currently appears; retain draft guards and existing specialized state controls.
2. SessionChatBoxContainer.tsx: resolve linked issue by workspace relationship, using useWorkspaceIssueComments, already cached by the conversation list. Pass simple_id to UI. Explicit prompt uses the same handleSend/buildAgentPrompt/send/completeSuccessfulSend path and draft guard.
3. SessionChatBox.test.tsx: rendered tests for exact prompts, whitespace, nonempty, unresolved identity, restoration, and modes.
4. Verify formatting, both frontend consumers and shared packages, component tests; independently review diff with Codex CLI.
No persisted model or API changes. UI contract: onSend(message?: string), issueSimpleId?: string. Avoid dependencies. Main risks: stale issue identity, draft bypass and footer crowding. Identity is selected by current workspace only; unresolved lookup disables What’s left; buttons are compact; ChatBoxBase.tsx lets the footer wrap and caps the action area at its available width.
Constitution: II tests contract, III/VI reuse submission, IV preserves package boundaries, XXXVII keeps controls reachable. No violations.
