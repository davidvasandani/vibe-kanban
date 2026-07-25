import type { PipelineFileStatus, PipelineParseError } from 'shared/types';

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
