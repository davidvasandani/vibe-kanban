import { describe, it, expect, vi } from 'vitest';
import type { BaseCodingAgent, ExecutorProfile } from 'shared/types';

vi.mock('@/shared/hooks/usePresetOptions', () => ({
  usePresetOptions: () => ({ data: undefined }),
}));

import { resolveEffectiveExecutor } from './useExecutorConfig';

const CLAUDE = 'CLAUDE_CODE' as BaseCodingAgent;
const CODEX = 'CODEX' as BaseCodingAgent;
const QWEN = 'QWEN_CODE' as BaseCodingAgent;

const profiles = {
  CLAUDE_CODE: { DEFAULT: { CLAUDE_CODE: {} } },
  CODEX: { DEFAULT: { CODEX: {} } },
  QWEN_CODE: { disabled: true, DEFAULT: { QWEN_CODE: {} } },
} as unknown as Record<string, ExecutorProfile>;

describe('resolveEffectiveExecutor', () => {
  it('hides disabled agents from the options', () => {
    const { effective, options } = resolveEffectiveExecutor(
      profiles,
      undefined,
      undefined,
      undefined,
      CODEX
    );
    expect(effective).toBe(CODEX);
    expect(options).toEqual([CLAUDE, CODEX]);
  });

  it('skips a disabled last-used agent and falls through to the default', () => {
    const { effective, options } = resolveEffectiveExecutor(
      profiles,
      undefined,
      undefined,
      QWEN,
      CODEX
    );
    expect(effective).toBe(CODEX);
    expect(options).not.toContain(QWEN);
  });

  it('skips a disabled default and uses the first enabled agent', () => {
    const { effective } = resolveEffectiveExecutor(
      profiles,
      undefined,
      undefined,
      undefined,
      QWEN
    );
    expect(effective).toBe(CLAUDE);
  });

  it('honours an explicit selection or draft and keeps it visible', () => {
    for (const [user, scratch] of [
      [QWEN, undefined],
      [undefined, QWEN],
    ] as const) {
      const { effective, options } = resolveEffectiveExecutor(
        profiles,
        user,
        scratch,
        CLAUDE,
        CODEX
      );
      expect(effective).toBe(QWEN);
      expect(options).toEqual([CLAUDE, CODEX, QWEN]);
    }
  });

  it('never leaves the picker empty when every agent is disabled', () => {
    const allDisabled = {
      CODEX: { disabled: true, DEFAULT: { CODEX: {} } },
      QWEN_CODE: { disabled: true, DEFAULT: { QWEN_CODE: {} } },
    } as unknown as Record<string, ExecutorProfile>;
    const { effective, options } = resolveEffectiveExecutor(
      allDisabled,
      undefined,
      undefined,
      undefined,
      undefined
    );
    expect(effective).toBe(CODEX);
    expect(options).toEqual([CODEX, QWEN]);
  });

  it('returns nothing without profiles', () => {
    expect(
      resolveEffectiveExecutor(null, undefined, undefined, undefined, undefined)
    ).toEqual({ effective: null, options: [] });
  });
});
