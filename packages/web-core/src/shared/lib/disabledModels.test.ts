import { describe, it, expect } from 'vitest';
import type {
  ExecutorProfile,
  ModelInfo,
  ModelSelectorConfig,
} from 'shared/types';
import {
  filterDisabledModels,
  getDisabledModelEntries,
  isModelDisabled,
  toggleDisabledModel,
  updateDisabledModels,
} from './disabledModels';
import { getExecutorVariantKeys } from './executor';

function model(id: string, providerId: string | null = null): ModelInfo {
  return { id, name: id, provider_id: providerId, reasoning_options: [] };
}

const opus = model('claude-opus-5-5');
const sonnet = model('claude-sonnet-5');
const haiku = model('claude-haiku-4-5');
const all = [opus, sonnet, haiku];

function config(models: ModelInfo[]): ModelSelectorConfig {
  return {
    providers: [],
    models,
    default_model: 'claude-opus-5-5',
    agents: [],
    permissions: [],
  } as ModelSelectorConfig;
}

describe('getDisabledModelEntries', () => {
  it('returns [] without profiles, executor or field', () => {
    expect(getDisabledModelEntries(null, 'CLAUDE_CODE')).toEqual([]);
    expect(getDisabledModelEntries({}, null)).toEqual([]);
    expect(getDisabledModelEntries({ CLAUDE_CODE: {} }, 'CLAUDE_CODE')).toEqual(
      []
    );
  });

  it('trims and drops blank entries', () => {
    const profiles = {
      CLAUDE_CODE: { disabled_models: [' claude-haiku-4-5 ', ''] },
    } as Record<string, ExecutorProfile>;
    expect(getDisabledModelEntries(profiles, 'CLAUDE_CODE')).toEqual([
      'claude-haiku-4-5',
    ]);
  });
});

describe('isModelDisabled', () => {
  it('matches case-insensitively and includes the provider', () => {
    expect(isModelDisabled(['CLAUDE-HAIKU-4-5'], haiku)).toBe(true);
    expect(isModelDisabled(['openai/gpt-5'], model('gpt-5', 'openai'))).toBe(
      true
    );
    expect(isModelDisabled(['gpt-5'], model('gpt-5', 'openai'))).toBe(false);
  });
});

describe('toggleDisabledModel', () => {
  it('disables an enabled model and re-enables it', () => {
    const disabled = toggleDisabledModel([], haiku, all);
    expect(disabled).toEqual(['claude-haiku-4-5']);
    expect(toggleDisabledModel(disabled, haiku, all)).toEqual([]);
  });

  it('refuses to disable the last enabled model', () => {
    const entries = ['claude-sonnet-5', 'claude-haiku-4-5'];
    expect(toggleDisabledModel(entries, opus, all)).toBe(entries);
  });
});

describe('updateDisabledModels', () => {
  it('sets the list without touching variants or recents', () => {
    const profiles = {
      CLAUDE_CODE: {
        recently_used_models: { models: ['claude-opus-5'] },
        DEFAULT: { CLAUDE_CODE: {} },
      },
      CODEX: { DEFAULT: { CODEX: {} } },
    } as unknown as Record<string, ExecutorProfile>;

    const next = updateDisabledModels(profiles, 'CLAUDE_CODE', [
      'claude-haiku-4-5',
    ]);
    expect(next.CLAUDE_CODE.disabled_models).toEqual(['claude-haiku-4-5']);
    expect(next.CLAUDE_CODE.recently_used_models).toEqual({
      models: ['claude-opus-5'],
    });
    expect(getExecutorVariantKeys(next.CLAUDE_CODE)).toEqual(['DEFAULT']);
    expect(next.CODEX).toBe(profiles.CODEX);
    expect(profiles.CLAUDE_CODE.disabled_models).toBeUndefined();
  });

  it('removes the field when the list becomes empty', () => {
    const profiles = {
      CLAUDE_CODE: { disabled_models: ['claude-haiku-4-5'] },
    } as Record<string, ExecutorProfile>;
    const next = updateDisabledModels(profiles, 'CLAUDE_CODE', []);
    expect('disabled_models' in next.CLAUDE_CODE).toBe(false);
  });
});

describe('filterDisabledModels', () => {
  it('returns the same config when nothing is disabled', () => {
    const base = config(all);
    expect(filterDisabledModels(base, [])).toBe(base);
    expect(filterDisabledModels(base, ['not-in-catalog'])).toBe(base);
  });

  it('hides disabled models', () => {
    const filtered = filterDisabledModels(config(all), ['claude-haiku-4-5']);
    expect(filtered.models.map((m) => m.id)).toEqual([
      'claude-opus-5-5',
      'claude-sonnet-5',
    ]);
  });

  it('drops providers whose models are all hidden', () => {
    const base = {
      ...config([
        model('gpt-5', 'openai'),
        model('claude-opus-5', 'anthropic'),
      ]),
      providers: [
        { id: 'openai', name: 'OpenAI' },
        { id: 'anthropic', name: 'Anthropic' },
      ],
    } as ModelSelectorConfig;
    const filtered = filterDisabledModels(base, ['openai/gpt-5']);
    expect(filtered.providers.map((p) => p.id)).toEqual(['anthropic']);
  });

  it('keeps the selected model visible even when disabled', () => {
    const filtered = filterDisabledModels(
      config(all),
      ['claude-opus-5-5', 'claude-haiku-4-5'],
      'claude-opus-5-5'
    );
    expect(filtered.models.map((m) => m.id)).toEqual([
      'claude-opus-5-5',
      'claude-sonnet-5',
    ]);
  });
});
