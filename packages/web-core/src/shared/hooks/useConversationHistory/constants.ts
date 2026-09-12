import type { PatchTypeWithKey } from './types';

export const MIN_INITIAL_ENTRIES = 10;
/**
 * How many completed processes to fetch at once when building a history
 * window.
 *
 * Each fetch makes the server reload a whole raw log and rerun the vendor
 * normalizer, so this bounds concurrent work on the server as much as it
 * bounds sockets in the browser. Small enough not to stampede a coordinator
 * that is also proxying live agent output; large enough that opening a long
 * conversation is not the sum of every turn in the window.
 */
export const HISTORY_FETCH_CONCURRENCY = 5;
export const REMAINING_BATCH_SIZE = 50;
export const MAX_RECENT_HISTORY_PROCESSES = 20;

export const makeLoadingPatch = (
  executionProcessId: string
): PatchTypeWithKey => ({
  type: 'NORMALIZED_ENTRY',
  content: {
    entry_type: {
      type: 'loading',
    },
    content: '',
    timestamp: null,
  },
  patchKey: `${executionProcessId}:loading`,
  executionProcessId,
});

export const nextActionPatch: (
  failed: boolean,
  execution_processes: number,
  needs_setup: boolean,
  setup_help_text?: string
) => PatchTypeWithKey = (
  failed,
  execution_processes,
  needs_setup,
  setup_help_text
) => ({
  type: 'NORMALIZED_ENTRY',
  content: {
    entry_type: {
      type: 'next_action',
      failed: failed,
      execution_processes: execution_processes,
      needs_setup: needs_setup,
      setup_help_text: setup_help_text ?? null,
    },
    content: '',
    timestamp: null,
  },
  patchKey: 'next_action',
  executionProcessId: '',
});
