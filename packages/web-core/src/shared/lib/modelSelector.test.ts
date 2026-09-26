import { describe, it, expect } from 'vitest';
import type { ModelSelectorConfig } from 'shared/types';
import { appendPresetModel, resolveModelAlias } from './modelSelector';

const config = {
  providers: [],
  models: [
    {
      id: 'claude-opus-5-5',
      name: 'Opus 5.5',
      provider_id: null,
      reasoning_options: [{ id: 'high', label: 'High', is_default: true }],
    },
  ],
  default_model: 'claude-opus-5-5',
  agents: [],
  permissions: [],
  model_aliases: { opus: 'claude-opus-5-5' },
} as ModelSelectorConfig;

describe('resolveModelAlias', () => {
  it('maps a saved alias to the versioned catalog model', () => {
    expect(resolveModelAlias(config, 'opus')).toBe('claude-opus-5-5');
    expect(resolveModelAlias(config, 'OPUS')).toBe('claude-opus-5-5');
  });

  it('passes through catalog IDs, unknown IDs and empty values', () => {
    expect(resolveModelAlias(config, 'claude-opus-5-5')).toBe(
      'claude-opus-5-5'
    );
    expect(resolveModelAlias(config, 'custom-model')).toBe('custom-model');
    expect(resolveModelAlias(config, null)).toBeNull();
    expect(resolveModelAlias(null, 'opus')).toBe('opus');
  });

  it('keeps the effort options when a preset names an alias', () => {
    const resolved = appendPresetModel(
      config,
      resolveModelAlias(config, 'opus')
    );
    expect(resolved?.models).toHaveLength(1);
    expect(resolved?.models[0].reasoning_options).toHaveLength(1);
  });
});
