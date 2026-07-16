# Implementation Plan: Grok Executor Support

1. **Research the official Grok Build contracts**
   - Inspect the current official CLI documentation/help for headless prompting, streaming JSON, session resume, permissions, authentication status, models, ACP, and MCP configuration.
   - Capture sanitized representative event fixtures and decide whether the native headless stream or the existing ACP executor abstraction provides the most complete integration.
   - Record version assumptions and forward-compatibility constraints in the SpecKit research output.

2. **Add the backend executor identity and configuration**
   - Add `Grok` to `CodingAgent`/`BaseCodingAgent` and all exhaustive capability/configuration mappings.
   - Define the serializable Grok executor settings using existing `AppendPrompt` and `CmdOverrides` conventions.
   - Add default profile/configuration creation and serialization round-trip coverage.

3. **Implement Grok command execution and lifecycle**
   - Build the official installed `grok` command for initial and follow-up turns with model/argument/environment overrides.
   - Run it in the task worktree with noninteractive permissions and structured output enabled.
   - Extract and persist the session ID, resume it for follow-ups, propagate cancellation/exit state, and provide actionable errors.
   - Implement executable/auth availability and setup guidance without handling secrets.

4. **Normalize Grok's event stream**
   - Add defensive serde event types with an unknown-event fallback.
   - Convert assistant, reasoning, tool, result, error, session, and usage events into normalized patches using the shared entry index.
   - Add sanitized fixture tests for incremental output, tool replacement, session extraction, failures, usage, malformed lines, and unknown events.

5. **Integrate Grok MCP configuration**
   - Add Grok's config path and TOML codec/adapter to native and shared MCP configuration flows.
   - Preserve unrelated Grok configuration keys and use existing atomic write/backup behavior.
   - Declare and test the CLI's supported transports and shared-gateway materialization.

6. **Complete frontend and API integration**
   - Regenerate shared TypeScript contracts from Rust.
   - Add Grok to executor labels, icons, selectors, settings forms/schemas, defaults, and other exhaustive frontend maps.
   - Update supported-agent and setup documentation with official installation/authentication instructions.

7. **Verify in layers**
   - Run focused Rust tests for the Grok executor, parser, profiles, and MCP adapter.
   - Run type generation checks and focused frontend checks/tests.
   - Run repository formatting, backend/frontend checks, lint, and relevant workspace tests; distinguish pre-existing failures from regressions.

8. **Review and close out**
   - Run the required independent Codex diff review and fix confirmed significant findings until clear.
   - Distill reusable Grok executor/event/MCP integration knowledge into the project knowledge base, tag it with task `43bc-add-grok-to-vk`, refresh the index, and commit the knowledge-base update.
