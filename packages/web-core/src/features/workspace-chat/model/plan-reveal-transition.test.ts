import { describe, expect, it } from 'vitest';

import type { PatchTypeWithKey } from '@/shared/hooks/useConversationHistory/types';

import {
  INITIAL_PLAN_REVEAL_STATE,
  coalesceConversationAddType,
  resolvePlanRevealTransition,
} from './plan-reveal-transition';

function planEntry(patchKey: string): PatchTypeWithKey {
  return {
    type: 'NORMALIZED_ENTRY',
    content: {
      entry_type: {
        type: 'tool_use',
        tool_name: 'ExitPlanMode',
        action_type: { action: 'unknown', data: null },
        status: { status: 'success' },
      },
      content: '',
      timestamp: null,
    },
    patchKey,
    executionProcessId: 'process-1',
  } as PatchTypeWithKey;
}

describe('resolvePlanRevealTransition', () => {
  it('reveals a newly observed plan exactly once across repeated snapshots', () => {
    const first = resolvePlanRevealTransition(
      'running',
      planEntry('process-1:4'),
      INITIAL_PLAN_REVEAL_STATE
    );
    const repeated = resolvePlanRevealTransition(
      'running',
      planEntry('process-1:4'),
      first.state
    );

    expect(first.addEntryType).toBe('plan');
    expect(first.state.lastRevealedPatchKey).toBe('process-1:4');
    expect(repeated.addEntryType).toBe('running');
    expect(repeated.state).toBe(first.state);
  });

  it('reveals a distinct later plan', () => {
    const transition = resolvePlanRevealTransition(
      'running',
      planEntry('process-2:9'),
      { lastRevealedPatchKey: 'process-1:4' }
    );

    expect(transition).toEqual({
      addEntryType: 'plan',
      state: { lastRevealedPatchKey: 'process-2:9' },
    });
  });

  it('preserves the incoming update type when the latest entry is not a plan', () => {
    const transition = resolvePlanRevealTransition('historic', undefined, {
      lastRevealedPatchKey: 'process-1:4',
    });

    expect(transition).toEqual({
      addEntryType: 'historic',
      state: { lastRevealedPatchKey: 'process-1:4' },
    });
  });

  it('reveals the same patch key again after conversation state resets', () => {
    const transition = resolvePlanRevealTransition(
      'initial',
      planEntry('process-1:4'),
      INITIAL_PLAN_REVEAL_STATE
    );

    expect(transition.addEntryType).toBe('plan');
  });
});

describe('coalesceConversationAddType', () => {
  it('keeps an unconsumed plan reveal while adopting a later snapshot', () => {
    expect(coalesceConversationAddType('plan', 'running')).toBe('plan');
  });

  it('uses the newest type when no plan reveal is pending', () => {
    expect(coalesceConversationAddType('running', 'historic')).toBe('historic');
  });
});
