# Clarifications: Three rollout loose ends
+
+Task: `vk/94c0-three-loose-ends`
+
+## 1. Does strict config work for pinned Codex app-server?
+
+**Decision:** Yes. The Cargo checkout resolved by the
+`rust-v0.144.1` pin defines `--strict-config` on app-server and includes an
+upstream integration test proving an unknown `config.toml` field makes startup
+fail with an identifying error. Vibe Kanban will append the flag after
+`app-server` and test the exact built command.
+
+Strict mode is process-wide in the app-server config manager and is carried into
+thread configuration loading. This is the appropriate fail-loud boundary for
+both native config and per-thread overrides.
+
+## 2. What was `include_apply_patch_tool` intended to do?
+
+**Decision:** It was a direct V1 protocol field retained during the 2025
+app-server migration. Commit `7c10c00d` passed it as a typed
+`NewConversationParams` field; the later V2 migration moved it into the generic
+config map even though pinned V2 `ConfigToml` has no such key. There is no
+verified current equivalent. Remove the setting and its generated schema surface
+rather than translate it into a guessed tool control.
+
+## 3. Which plural categories should be added?
+
+**Decision:** Keep `_one` and `_other` in all six locale files. That is the
+existing convention already used in every shipped `common.json`, including
+Japanese, Korean, and both Chinese variants. Do not invent categories unused by
+the current locale resources.
+
+## Remaining questions
+
+None.
