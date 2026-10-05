import { describe, it, expect } from 'vitest';
import type { BaseCodingAgent, ExecutorProfile } from 'shared/types';
import {
  filterEnabledAgents,
  getDisableAgentBlocker,
  isAgentDisabled,
  setAgentDisabled,
} from './disabledAgents';
import { getExecutorVariantKeys } from './executor';

const profiles = {
  CLAUDE_CODE: { DEFAULT: { CLAUDE_CODE: {} } },
  CODEX: { DEFAULT: { CODEX: {} } },
  QWEN_CODE: { disabled: true, DEFAULT: { QWEN_CODE: {} } },
  GROK: { disabled: true, DEFAULT: { GROK: {} } },
} as unknown as Record<string, ExecutorProfile>;

const agents = ['CLAUDE_CODE', 'CODEX', 'QWEN_CODE', 'GROK'];

describe('isAgentDisabled', () => {
  it('is true only for an explicit disabled flag', () => {
    expect(isAgentDisabled(profiles, 'QWEN_CODE')).toBe(true);
    expect(isAgentDisabled(profiles, 'CLAUDE_CODE')).toBe(false);
    expect(isAgentDisabled(profiles, 'MISSING')).toBe(false);
    expect(isAgentDisabled(null, 'QWEN_CODE')).toBe(false);
    expect(isAgentDisabled(profiles, null)).toBe(false);
  });
});

describe('filterEnabledAgents', () => {
  it('drops disabled agents and keeps order', () => {
    expect(filterEnabledAgents(agents, profiles)).toEqual([
      'CLAUDE_CODE',
      'CODEX',
    ]);
  });

  it('keeps the current selection visible even when disabled', () => {
    expect(filterEnabledAgents(agents, profiles, ['GROK', null])).toEqual([
      'CLAUDE_CODE',
      'CODEX',
      'GROK',
    ]);
  });

  it('returns every agent rather than an empty list', () => {
    const allDisabled = {
      QWEN_CODE: { disabled: true },
      GROK: { disabled: true },
    };
    expect(filterEnabledAgents(['QWEN_CODE', 'GROK'], allDisabled)).toEqual([
      'QWEN_CODE',
      'GROK',
    ]);
    expect(
      filterEnabledAgents(['QWEN_CODE', 'GROK'], allDisabled, ['GROK'])
    ).toEqual(['QWEN_CODE', 'GROK']);
  });

  it('is a no-op without profiles', () => {
    expect(filterEnabledAgents(agents, null)).toEqual(agents);
  });
});

describe('setAgentDisabled', () => {
  it('sets the flag and preserves the rest of the profile', () => {
    const next = setAgentDisabled(profiles, 'CODEX' as BaseCodingAgent, true);
    expect(next.CODEX?.disabled).toBe(true);
    expect(getExecutorVariantKeys(next.CODEX)).toEqual(['DEFAULT']);
    expect(profiles.CODEX?.disabled).toBeUndefined();
  });

  it('removes the key when re-enabling', () => {
    const next = setAgentDisabled(
      profiles,
      'QWEN_CODE' as BaseCodingAgent,
      false
    );
    expect(next.QWEN_CODE).not.toHaveProperty('disabled');
    expect(getExecutorVariantKeys(next.QWEN_CODE)).toEqual(['DEFAULT']);
  });
});

describe('getDisableAgentBlocker', () => {
  it('blocks the default agent', () => {
    expect(getDisableAgentBlocker(profiles, 'CODEX', 'CODEX')).toBe('default');
  });

  it('blocks the last enabled agent', () => {
    const oneLeft = setAgentDisabled(
      profiles,
      'CODEX' as BaseCodingAgent,
      true
    );
    expect(getDisableAgentBlocker(oneLeft, 'CLAUDE_CODE', null)).toBe('last');
  });

  it('allows disabling any other enabled agent', () => {
    expect(getDisableAgentBlocker(profiles, 'CODEX', 'CLAUDE_CODE')).toBe(null);
  });

  it('never blocks re-enabling', () => {
    expect(getDisableAgentBlocker(profiles, 'GROK', 'GROK')).toBe(null);
  });
});

describe('getExecutorVariantKeys', () => {
  it('does not list disabled as a configuration', () => {
    expect(getExecutorVariantKeys(profiles.QWEN_CODE)).toEqual(['DEFAULT']);
  });
});
