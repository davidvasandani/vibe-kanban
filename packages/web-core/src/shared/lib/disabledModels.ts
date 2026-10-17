import type {
  BaseCodingAgent,
  ExecutorProfile,
  ModelInfo,
  ModelSelectorConfig,
} from 'shared/types';
import { getModelKey } from '@/shared/lib/recentModels';

type ProfilesMap = Partial<Record<string, ExecutorProfile>> | null | undefined;

export function getDisabledModelEntries(
  profiles: ProfilesMap,
  executor: BaseCodingAgent | null | undefined
): string[] {
  if (!profiles || !executor) return [];
  const entries = profiles[executor]?.disabled_models ?? [];
  return entries.map((e) => e.trim()).filter(Boolean);
}

export function isModelDisabled(entries: string[], model: ModelInfo): boolean {
  const key = getModelKey(model).toLowerCase();
  return entries.some((e) => e.toLowerCase() === key);
}

/**
 * Flip a model between enabled and disabled. Refuses to disable the last
 * enabled model in `allModels` so the picker is never left empty.
 */
export function toggleDisabledModel(
  entries: string[],
  model: ModelInfo,
  allModels: ModelInfo[]
): string[] {
  const keyLower = getModelKey(model).toLowerCase();
  if (isModelDisabled(entries, model)) {
    return entries.filter((e) => e.toLowerCase() !== keyLower);
  }
  const enabledCount = allModels.filter(
    (m) => !isModelDisabled(entries, m)
  ).length;
  if (enabledCount <= 1) return entries;
  return [...entries, getModelKey(model)];
}

export function updateDisabledModels<
  T extends Partial<Record<string, ExecutorProfile>>,
>(profiles: T, executor: BaseCodingAgent, entries: string[]): T {
  const normalized = entries.map((e) => e.trim()).filter(Boolean);
  const nextProfile = { ...(profiles[executor] ?? {}) } as ExecutorProfile;
  if (normalized.length > 0) {
    nextProfile.disabled_models = normalized;
  } else {
    delete nextProfile.disabled_models;
  }
  return { ...profiles, [executor]: nextProfile };
}

/**
 * Hide disabled models from a picker config. The model identified by
 * `keepModelKey` (the current selection) always stays visible so the trigger
 * never names a model that is missing from its own menu.
 */
export function filterDisabledModels(
  config: ModelSelectorConfig,
  entries: string[],
  keepModelKey?: string | null
): ModelSelectorConfig {
  if (entries.length === 0) return config;
  const keepLower = keepModelKey?.toLowerCase() ?? null;
  const models = config.models.filter(
    (model) =>
      !isModelDisabled(entries, model) ||
      getModelKey(model).toLowerCase() === keepLower
  );
  if (models.length === config.models.length) return config;
  // Drop provider groups whose models were all hidden.
  const remainingProviders = new Set(
    models.map((m) => m.provider_id?.toLowerCase()).filter(Boolean)
  );
  const providers = config.providers.filter((p) =>
    remainingProviders.has(p.id.toLowerCase())
  );
  return { ...config, models, providers };
}
