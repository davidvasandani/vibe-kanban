# Technical Plan: Add Claude Opus 5 to Executor Model Selectors

## Scope

This plan covers adding `claude-opus-5` to the four affected executor model
catalogs, updating schema-carrying metadata annotations, regenerating artifacts,
and extending tests. It does **not** cover CLI version bumps, default model
changes, or fast-mode variants.

---

## 1. Source Analysis Summary

### Files to modify (Rust source)

| Executor | File | Key sections |
|----------|------|-------------|
| Claude Code | `crates/executors/src/executors/claude.rs` | `default_discovered_options()` (lines 266-314) — model catalog array |
| Cursor | `crates/executors/src/executors/cursor.rs` | `resolve_cursor_model_name()` (lines 60-112), `cursor_reasoning_options()` (lines 114-139), `CursorAgent` struct `#[schemars(description)]` (line 50), `discover_options()` model array (lines 651-686) |
| Copilot | `crates/executors/src/executors/copilot.rs` | `discover_options()` model array (lines 199-219) |
| Droid | `crates/executors/src/executors/droid.rs` | `Droid` struct `#[schemars(description)]` (line 73), `discover_options()` model array (lines 240-262) |

### Generated artifacts to regenerate

| Artifact | Source trigger | Regeneration command |
|----------|---------------|---------------------|
| `shared/schemas/cursor_agent.json` | `#[schemars(description)]` on `CursorAgent.model` changes | `pnpm run generate-types` |
| `shared/schemas/droid.json` | `#[schemars(description)]` on `Droid.model` changes | `pnpm run generate-types` |
| `shared/types.ts` | No struct-level changes expected (no new fields) — but must verify | `pnpm run generate-types` |

**Note:** `shared/schemas/claude_code.json` and `shared/schemas/copilot.json`
have **no** `#[schemars(description)]` annotation listing model IDs on their
`model` fields. The Claude Code `model` field has no description at all;
Copilot's also has none. These schemas will not change from this task.

---

## 2. Change Plan by Executor

### 2.1 Claude Code (`claude.rs`)

**What:** Add one entry to the model catalog in `default_discovered_options()`.

**Current catalog (lines 281-288):**
```rust
("opus", "Opus"),
("opus[1m]", "Opus (1M context)"),
("claude-sonnet-5", "Sonnet 5"),
("sonnet", "Sonnet"),
("fable", "Fable"),
("haiku", "Haiku"),
```

**Change:** Insert `("claude-opus-5", "Opus 5")` after `("opus[1m]", "Opus (1M context)")` and before `("claude-sonnet-5", "Sonnet 5")`.

**Resulting catalog:**
```rust
("opus", "Opus"),
("opus[1m]", "Opus (1M context)"),
("claude-opus-5", "Opus 5"),
("claude-sonnet-5", "Sonnet 5"),
("sonnet", "Sonnet"),
("fable", "Fable"),
("haiku", "Haiku"),
```

**Reasoning options:** The existing `supports_effort` closure (line 275-276)
checks `id.contains("opus")`. The string `"claude-opus-5"` contains `"opus"`,
so reasoning effort options are automatically granted. **No logic change
needed.**

**Schema impact:** None. `ClaudeCode.model` has no `#[schemars(description)]`
listing model IDs. `shared/schemas/claude_code.json` will not change.

---

### 2.2 Cursor (`cursor.rs`)

**What:** Four coordinated changes.

#### 2.2a. Description string (`CursorAgent` struct, line 50)

**Current:**
```
"auto, opus-4.8, opus-4.6, sonnet-4.6, gpt-5.4, ..."
```

**Change:** Insert `opus-5` after `auto` and before `opus-4.8`:
```
"auto, opus-5, opus-4.8, opus-4.6, sonnet-4.6, gpt-5.4, ..."
```

#### 2.2b. Model name resolution (`resolve_cursor_model_name`, lines 99-108)

**Current Claude arms:**
```rust
("opus-4.8", Some("standard")) => "opus-4.8",
("opus-4.8", Some("thinking") | None) => "opus-4.8-thinking",
("opus-4.6", Some("standard")) => "opus-4.6",
("opus-4.6", Some("thinking") | None) => "opus-4.6-thinking",
...
```

**Change:** Add two arms before the `opus-4.8` arms:
```rust
("opus-5", Some("standard")) => "opus-5",
("opus-5", Some("thinking") | None) => "opus-5-thinking",
```

#### 2.2c. Reasoning options (`cursor_reasoning_options`, line 125)

**Current match arm:**
```rust
"opus-4.8" | "opus-4.6" | "sonnet-4.6" | "opus-4.5" | "sonnet-4.5" => vec![...]
```

**Change:** Prepend `"opus-5"`:
```rust
"opus-5" | "opus-4.8" | "opus-4.6" | "sonnet-4.6" | "opus-4.5" | "sonnet-4.5" => vec![...]
```

#### 2.2d. Model catalog (`discover_options`, lines 651-678)

**Change:** Insert `("opus-5", "Claude 5 Opus")` before `("opus-4.8", "Claude 4.8 Opus")` (line 655):
```rust
("opus-5", "Claude 5 Opus"),
("opus-4.8", "Claude 4.8 Opus"),
```

**Schema impact:** The `#[schemars(description)]` change triggers regeneration
of `shared/schemas/cursor_agent.json`.

---

### 2.3 Copilot (`copilot.rs`)

**What:** Add one entry to the model catalog.

**Change:** Insert `("claude-opus-5", "Claude Opus 5")` before
`("claude-opus-4.8", "Claude Opus 4.8")` (line 201):
```rust
("claude-opus-5", "Claude Opus 5"),
("claude-opus-4.8", "Claude Opus 4.8"),
```

**Schema impact:** None. `Copilot.model` has no `#[schemars(description)]`
annotation. `shared/schemas/copilot.json` will not change.

---

### 2.4 Droid (`droid.rs`)

**What:** Two changes.

#### 2.4a. Description string (`Droid` struct, line 73)

**Current:**
```
"Model to use (e.g., gpt-5-codex, claude-sonnet-4-5-20250929, gpt-5-2025-08-07, claude-opus-4-1-20250805, claude-haiku-4-5-20251001, glm-4.6)"
```

**Change:** Add `claude-opus-5` to the examples list:
```
"Model to use (e.g., claude-opus-5, gpt-5-codex, claude-sonnet-4-5-20250929, gpt-5-2025-08-07, claude-opus-4-1-20250805, claude-haiku-4-5-20251001, glm-4.6)"
```

#### 2.4b. Model catalog (`discover_options`, lines 241-261)

**Change:** Insert `("claude-opus-5", "Claude Opus 5")` before
`("claude-opus-4-8", "Claude Opus 4.8")` (line 241):
```rust
("claude-opus-5", "Claude Opus 5"),
("claude-opus-4-8", "Claude Opus 4.8"),
```

**Schema impact:** The `#[schemars(description)]` change triggers regeneration
of `shared/schemas/droid.json`.

---

## 3. Generated Artifact Regeneration

After all Rust source edits:

```bash
pnpm run generate-types
```

This regenerates both `shared/types.ts` and all `shared/schemas/*.json`.

**Expected diffs in generated files:**
- `shared/schemas/cursor_agent.json` — `description` field for `model` gains
  `opus-5` in the list
- `shared/schemas/droid.json` — `description` field for `model` gains
  `claude-opus-5` in examples

**Expected no-change in:**
- `shared/schemas/claude_code.json` (no model description annotation)
- `shared/schemas/copilot.json` (no model description annotation)
- `shared/types.ts` (no new Rust struct fields)
- All other schemas (`amp.json`, `codex.json`, `gemini.json`, etc.)

**Verification:**
```bash
pnpm run generate-types:check
```

---

## 4. Testing Plan

### 4.1 New/Extended Unit Tests

#### Claude Code test (`claude.rs`)

Add a test in the existing `mod tests` (after line 2860) that verifies:
- `default_discovered_options()` model list contains `"claude-opus-5"`
- The `"claude-opus-5"` entry has non-empty reasoning options (confirming
  `supports_effort` coverage)

```rust
#[test]
fn test_opus_5_in_model_catalog() {
    let options = default_discovered_options();
    let opus5 = options.model_selector.models
        .iter()
        .find(|m| m.id == "claude-opus-5");
    assert!(opus5.is_some(), "claude-opus-5 must be in the model catalog");
    let opus5 = opus5.unwrap();
    assert_eq!(opus5.name, "Opus 5");
    assert!(!opus5.reasoning_options.is_empty(),
        "claude-opus-5 should have reasoning options via supports_effort");
}
```

#### Cursor tests (`cursor.rs`)

Add tests in the existing `mod tests` (after line 1403):

```rust
#[test]
fn test_opus_5_reasoning_resolution() {
    assert_eq!(resolve_cursor_model_name("opus-5", Some("standard")), "opus-5");
    assert_eq!(resolve_cursor_model_name("opus-5", Some("thinking")), "opus-5-thinking");
    assert_eq!(resolve_cursor_model_name("opus-5", None), "opus-5-thinking");
}

#[test]
fn test_opus_5_reasoning_options() {
    let options = cursor_reasoning_options("opus-5");
    assert_eq!(options.len(), 2);
    assert!(options.iter().any(|o| o.id == "standard"));
    assert!(options.iter().any(|o| o.id == "thinking"));
}

#[tokio::test]
async fn test_opus_5_in_cursor_catalog() {
    let agent = CursorAgent {
        append_prompt: AppendPrompt::default(),
        force: None,
        model: None,
        reasoning: None,
        cmd: Default::default(),
    };
    let stream = agent.discover_options(None, None).await.unwrap();
    // Verify opus-5 exists in the discovered model list
    use futures::StreamExt;
    let patches: Vec<_> = stream.collect().await;
    let json = serde_json::to_string(&patches).unwrap();
    assert!(json.contains("opus-5"), "opus-5 must appear in discovered options");
}
```

#### Copilot test (`copilot.rs`)

No existing test module. Add one:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_opus_5_in_copilot_catalog() {
        let copilot = Copilot {
            append_prompt: AppendPrompt::default(),
            model: None,
            allow_all_tools: None,
            allow_tool: None,
            deny_tool: None,
            add_dir: None,
            disable_mcp_server: None,
            cmd: Default::default(),
            approvals: None,
        };
        let stream = copilot.discover_options(None, None).await.unwrap();
        use futures::StreamExt;
        let patches: Vec<_> = stream.collect().await;
        let json = serde_json::to_string(&patches).unwrap();
        assert!(json.contains("claude-opus-5"),
            "claude-opus-5 must appear in Copilot discovered options");
    }
}
```

#### Droid test

No existing test module in `droid.rs`. Add one:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_opus_5_in_droid_catalog() {
        let droid = Droid {
            append_prompt: AppendPrompt::default(),
            autonomy: Autonomy::Normal,
            model: None,
            reasoning_effort: None,
            cmd: Default::default(),
        };
        let stream = droid.discover_options(None, None).await.unwrap();
        use futures::StreamExt;
        let patches: Vec<_> = stream.collect().await;
        let json = serde_json::to_string(&patches).unwrap();
        assert!(json.contains("claude-opus-5"),
            "claude-opus-5 must appear in Droid discovered options");
    }
}
```

### 4.2 Verification Commands

```bash
# 1. Compile all workspaces
cargo build --workspace

# 2. Run all tests
cargo test --workspace

# 3. Verify generated types/schemas are consistent
pnpm run generate-types:check

# 4. Format
pnpm run format

# 5. Lint
pnpm run lint
```

---

## 5. Execution Order

1. **Edit `claude.rs`** — add `("claude-opus-5", "Opus 5")` to catalog + add test
2. **Edit `cursor.rs`** — all four changes (description, resolution, reasoning
   options, catalog) + add tests
3. **Edit `copilot.rs`** — add catalog entry + add test
4. **Edit `droid.rs`** — update description, add catalog entry + add test
5. **Regenerate** — `pnpm run generate-types`
6. **Verify** — `cargo test --workspace && pnpm run generate-types:check`
7. **Format & lint** — `pnpm run format && pnpm run lint`

Steps 1-4 are independent of each other and can be done in any order. Step 5
must follow all source edits. Steps 6-7 are final verification.

---

## 6. Risk Assessment

| Risk | Mitigation |
|------|-----------|
| `supports_effort` closure doesn't match `"claude-opus-5"` | Verified: `"claude-opus-5".contains("opus")` is true. Test confirms. |
| Cursor reasoning resolution falls through to wildcard | New match arms added before wildcard `_ => base_model`. Test confirms. |
| Schema regeneration misses a file | `generate-types:check` CI gate catches drift. |
| Copilot dot vs Droid hyphen confusion for version 5 | Both produce `"claude-opus-5"` — no ambiguity for integer versions. Noted in spec. |
| Ordering breaks existing model selection | Additive insertion only; no reordering of existing entries. |

---

## 7. Data Model / Contract Notes

- **No new Rust struct fields** — only array literal changes and match arms.
- **No API contract changes** — the `ModelInfo` struct shape is unchanged;
  only the runtime values returned by `discover_options()` gain one more entry.
- **No database migration** — model selection is ephemeral, not persisted.
- **No frontend changes** — the UI consumes `discover_options()` generically;
  new models appear automatically in the model picker.
