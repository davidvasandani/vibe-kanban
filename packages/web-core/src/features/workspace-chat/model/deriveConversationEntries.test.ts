import { describe, expect, it } from 'vitest';
import type { ExecutorAction } from 'shared/types';

import type { ConversationTimelineSource } from '@/shared/hooks/useConversationHistory/types';
import { deriveConversationEntries } from './deriveConversationEntries';

const createdAt = '2026-09-16T10:15:00Z';

function source(executorAction: ExecutorAction): ConversationTimelineSource {
  return {
    executionProcessState: {
      process: {
        executionProcess: {
          id: 'process',
          created_at: createdAt,
          updated_at: createdAt,
          executor_action: executorAction,
        },
        entries: [],
      },
    },
    liveExecutionProcesses: [],
  };
}

describe('deriveConversationEntries timestamp provenance', () => {
  it('uses the process creation time for a derived user message', () => {
    const result = deriveConversationEntries({
      source: source({
        typ: {
          type: 'CodingAgentFollowUpRequest',
          prompt: 'Follow up',
          executor_config: {} as never,
          session_id: null,
          working_dir: null,
        },
      } as ExecutorAction),
      scriptOutputCache: new Map(),
    });

    const userMessage = result.entries.find(
      (entry) =>
        entry.type === 'NORMALIZED_ENTRY' &&
        entry.content.entry_type.type === 'user_message'
    );
    expect(
      userMessage?.type === 'NORMALIZED_ENTRY'
        ? userMessage.content.timestamp
        : null
    ).toBe(createdAt);
  });

  it('uses the process creation time for a derived script action', () => {
    const result = deriveConversationEntries({
      source: source({
        typ: {
          type: 'ScriptRequest',
          script: 'pnpm test',
          context: 'CleanupScript',
          next_action: null,
        },
      } as ExecutorAction),
      scriptOutputCache: new Map(),
    });

    const script = result.entries.find(
      (entry) =>
        entry.type === 'NORMALIZED_ENTRY' &&
        entry.content.entry_type.type === 'tool_use'
    );
    expect(
      script?.type === 'NORMALIZED_ENTRY' ? script.content.timestamp : null
    ).toBe(createdAt);
  });
});
