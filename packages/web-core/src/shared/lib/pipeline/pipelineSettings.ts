import type {
  NewIssuePipelineDefaults,
  PipelineFileStatus,
  PipelineParseError,
} from 'shared/types';

export const BUNDLED_PIPELINE_IDS = [
  'basic',
  'wikillm',
  'speckit',
  'parallel-subagents',
] as const;

export type BundledPipelineId = (typeof BUNDLED_PIPELINE_IDS)[number];

export type PipelineValidationTuple = {
  scopeKey: readonly string[];
  id: string;
  content: string;
};

const PIPELINE_ID_PATTERN = /^[A-Za-z0-9_-]+$/;

export function isValidPipelineId(id: string): boolean {
  return PIPELINE_ID_PATTERN.test(id);
}

export function isBundledPipelineId(id: string): id is BundledPipelineId {
  return BUNDLED_PIPELINE_IDS.includes(id as BundledPipelineId);
}

export function createPipelineStarterToml(id: string): string {
  const escapedName = id.replace(/\\/g, '\\\\').replace(/"/g, '\\"');
  return `name = "${escapedName}"
description = ""

[[stage]]
id = "stage-1"
label = "Stage 1"
prompt = "Describe what this stage should do."
`;
}

export function formatPipelineErrorLocation(
  error: Pick<PipelineParseError, 'line' | 'column'> | null | undefined
): string | null {
  if (error?.line == null || error.column == null) {
    return null;
  }

  return `${error.line}:${error.column}`;
}

export function validationTupleMatches(
  left: PipelineValidationTuple | null | undefined,
  right: PipelineValidationTuple | null | undefined
): boolean {
  if (!left || !right) {
    return false;
  }

  return (
    left.id === right.id &&
    left.content === right.content &&
    left.scopeKey.length === right.scopeKey.length &&
    left.scopeKey.every((segment, index) => segment === right.scopeKey[index])
  );
}

export function selectPipelineAfterRefresh(
  statuses: readonly PipelineFileStatus[] | null | undefined,
  currentId: string | null | undefined
): string | null {
  if (!statuses || statuses.length === 0) {
    return null;
  }

  if (currentId && statuses.some((status) => status.id === currentId)) {
    return currentId;
  }

  return statuses[0]?.id ?? null;
}

/** Which override map of `NewIssuePipelineDefaults` a scope edits. */
export type PipelineDefaultsScope = 'organizations' | 'projects';

/**
 * Add or remove `id` from `ids`. The result follows `order` (the pipeline
 * catalog order) so the stored list, and therefore the composed block,
 * doesn't depend on click order; ids not in `order` (missing pipelines)
 * keep their place at the end until removed.
 */
export function togglePipelineId(
  ids: readonly string[],
  id: string,
  enabled: boolean,
  order: readonly string[]
): string[] {
  const next = new Set(ids);
  if (enabled) next.add(id);
  else next.delete(id);
  const known = order.filter((o) => next.has(o));
  const unknown = [...next].filter((o) => !order.includes(o));
  return [...known, ...unknown];
}

export function toggleMachineDefault(
  defaults: NewIssuePipelineDefaults,
  id: string,
  enabled: boolean,
  order: readonly string[]
): NewIssuePipelineDefaults {
  return {
    ...defaults,
    pipeline_ids: togglePipelineId(defaults.pipeline_ids, id, enabled, order),
  };
}

/** The override for `scopeId`, or `undefined` when it inherits. */
export function getScopeOverride(
  defaults: NewIssuePipelineDefaults,
  scope: PipelineDefaultsScope,
  scopeId: string
): string[] | undefined {
  return defaults[scope][scopeId];
}

/** Set (or replace) an override. An empty list means "no pipelines". */
export function setScopeOverride(
  defaults: NewIssuePipelineDefaults,
  scope: PipelineDefaultsScope,
  scopeId: string,
  ids: readonly string[]
): NewIssuePipelineDefaults {
  return { ...defaults, [scope]: { ...defaults[scope], [scopeId]: [...ids] } };
}

/** Remove an override so the scope inherits again. */
export function clearScopeOverride(
  defaults: NewIssuePipelineDefaults,
  scope: PipelineDefaultsScope,
  scopeId: string
): NewIssuePipelineDefaults {
  const rest = Object.fromEntries(
    Object.entries(defaults[scope]).filter(([key]) => key !== scopeId)
  );
  return { ...defaults, [scope]: rest };
}

export function toggleScopePipeline(
  defaults: NewIssuePipelineDefaults,
  scope: PipelineDefaultsScope,
  scopeId: string,
  id: string,
  enabled: boolean,
  order: readonly string[]
): NewIssuePipelineDefaults {
  const current = getScopeOverride(defaults, scope, scopeId) ?? [];
  return setScopeOverride(
    defaults,
    scope,
    scopeId,
    togglePipelineId(current, id, enabled, order)
  );
}
