# Contract: `ExecutorProfile.disabled`

## JSON (`profiles.json`, `GET/PUT /api/profiles` content, `GET /api/info` → `executors`)

```json
{
  "executors": {
    "QWEN_CODE": {
      "disabled": true,
      "DEFAULT": { "QWEN_CODE": { "yolo": true } }
    }
  }
}
```

- `disabled` is optional. Absent or `false` means enabled.
- Writers omit it when `false`. Readers accept either.
- It is a reserved key: it is never a configuration/variant name.

## TypeScript (generated, `shared/types.ts`)

```ts
export type ExecutorProfile = {
  recently_used_models?: ExecutorRecentModels;
  disabled_models?: Array<string>;
  /** Hidden from agent pickers. ... */
  disabled?: boolean;
} & { [key in string]?: CodingAgent };
```
(Exact formatting comes from ts-rs. Shape only.)

## Frontend helper API (`web-core/src/shared/lib/disabledAgents.ts`)

```ts
isAgentDisabled(profiles, agent): boolean
filterEnabledAgents<T extends string>(agents: T[], profiles, keep?: (T | null | undefined)[]): T[]
setAgentDisabled(profiles, agent, disabled): profiles
getDisableAgentBlocker(profiles, agent, defaultAgent): 'default' | 'last' | null
```

## Behavioural guarantees
- Disabling never changes what a running or existing execution uses.
- No endpoint rejects a request because an agent is disabled.
