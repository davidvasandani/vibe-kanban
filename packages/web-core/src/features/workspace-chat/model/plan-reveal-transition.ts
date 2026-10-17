import type {
  AddEntryType,
  PatchTypeWithKey,
} from '@/shared/hooks/useConversationHistory/types';

export interface PlanRevealState {
  readonly lastRevealedPatchKey: string | null;
}

export interface PlanRevealTransition {
  readonly addEntryType: AddEntryType;
  readonly state: PlanRevealState;
}

export const INITIAL_PLAN_REVEAL_STATE: PlanRevealState = {
  lastRevealedPatchKey: null,
};

function isExitPlanModeEntry(
  entry: PatchTypeWithKey | undefined
): entry is PatchTypeWithKey {
  return (
    entry?.type === 'NORMALIZED_ENTRY' &&
    entry.content.entry_type.type === 'tool_use' &&
    entry.content.entry_type.tool_name === 'ExitPlanMode'
  );
}

/**
 * Convert plan detection from a snapshot-level condition into a logical edge.
 * The same stable entry can end many streaming snapshots, but it should request
 * plan navigation only on its first observation in the conversation scope.
 */
export function resolvePlanRevealTransition(
  addEntryType: AddEntryType,
  latestEntry: PatchTypeWithKey | undefined,
  state: PlanRevealState
): PlanRevealTransition {
  if (
    !isExitPlanModeEntry(latestEntry) ||
    latestEntry.patchKey === state.lastRevealedPatchKey
  ) {
    return { addEntryType, state };
  }

  return {
    addEntryType: 'plan',
    state: { lastRevealedPatchKey: latestEntry.patchKey },
  };
}

/** Preserve a one-shot plan reveal while animation-frame batching adopts the
 * newest timeline snapshot. The later snapshot data wins, but its ordinary
 * update type must not erase navigation that has not rendered yet. */
export function coalesceConversationAddType(
  pendingAddEntryType: AddEntryType | undefined,
  nextAddEntryType: AddEntryType
): AddEntryType {
  return pendingAddEntryType === 'plan' ? 'plan' : nextAddEntryType;
}
