import { describe, expect, it } from 'vitest';
import type {
  DeliveryTargetCandidate,
  IssueAgentTargets,
  IssueCommentDelivery,
  IssueCommentWithDelivery,
  WorkspaceIssueComments,
} from 'shared/types';
import type { PatchTypeWithKey } from '@/shared/hooks/useConversationHistory/types';
import {
  defaultCommentIntent,
  describeDelivery,
  mergeIssueComments,
  standaloneCommentId,
} from './issueCommentDelivery';

const candidate: DeliveryTargetCandidate = {
  workspace_id: 'w1',
  workspace_name: 'AMT MCP',
  workspace_branch: 'vk/amt',
  session_id: 's1',
  session_name: null,
  agent_state: 'waiting',
  latest_execution_id: 'e0',
};

function targets(
  overrides: Partial<IssueAgentTargets> = {}
): IssueAgentTargets {
  return {
    issue_id: 'i1',
    issue_active: true,
    candidates: [candidate],
    default_session_id: 's1',
    unavailable_reason: null,
    ...overrides,
  };
}

function delivery(
  overrides: Partial<IssueCommentDelivery> = {}
): IssueCommentDelivery {
  return {
    id: 'd1',
    comment_id: 'c1',
    issue_id: 'i1',
    workspace_id: 'w1',
    session_id: 's1',
    execution_process_id: null,
    state: 'pending',
    reason: null,
    error: null,
    source: 'ui',
    origin_session_id: null,
    hop_count: 0,
    comment_author_id: null,
    comment_created_at: '2026-10-19T04:45:00Z',
    delivered_message: null,
    delivered_comment_updated_at: null,
    attempt_count: 0,
    next_attempt_at: null,
    allow_stopped_target: false,
    created_at: '2026-10-19T04:45:00Z',
    updated_at: '2026-10-19T04:45:00Z',
    delivered_at: null,
    ...overrides,
  };
}

describe('defaultCommentIntent', () => {
  it('defaults to reply only with one eligible session', () => {
    expect(defaultCommentIntent(targets())).toBe('reply');
    expect(defaultCommentIntent(targets({ default_session_id: null }))).toBe(
      'note'
    );
    expect(defaultCommentIntent(undefined)).toBe('note');
  });
});

describe('describeDelivery', () => {
  it('never presents a saved comment as delivered', () => {
    const notSent = describeDelivery({ delivery: null, targets: targets() });
    expect(notSent?.labelKey).toBe('notSent');
    expect(notSent?.actions).toEqual(['send']);
    // Nothing linked: say nothing rather than imply a delivery path.
    expect(
      describeDelivery({
        delivery: null,
        targets: targets({ candidates: [] }),
      })
    ).toBeNull();
  });

  it('names the session and keeps backend diagnostics verbatim', () => {
    const queued = describeDelivery({
      delivery: delivery({ reason: 'agent_running' }),
      targets: targets(),
    });
    expect(queued?.labelKey).toBe('queued');
    expect(queued?.params.session).toBe('AMT MCP');

    const failed = describeDelivery({
      delivery: delivery({
        state: 'failed',
        reason: 'start_failed',
        error: 'could not start a turn:\nworker offline',
      }),
      targets: targets(),
    });
    expect(failed?.detail).toBe('could not start a turn:\nworker offline');
    expect(failed?.actions).toEqual(['retry', 'choose_target']);
  });

  it('offers target choice for ambiguous deliveries and flags edits', () => {
    expect(
      describeDelivery({
        delivery: delivery({ state: 'needs_target', reason: 'ambiguous' }),
        targets: targets(),
      })?.actions
    ).toEqual(['choose_target']);
    const edited = describeDelivery({
      delivery: delivery({ state: 'delivered', execution_process_id: 'e1' }),
      editedAfterDelivery: true,
      targets: targets(),
    });
    expect(edited?.labelKey).toBe('delivered');
    expect(edited?.editedAfterDelivery).toBe(true);
  });

  it('surfaces a failed delivery request with retry', () => {
    const model = describeDelivery({
      delivery: null,
      requestError: 'Bad gateway',
      targets: targets(),
    });
    expect(model?.labelKey).toBe('requestFailed');
    expect(model?.actions).toEqual(['retry']);
  });
});

function userEntry(executionId: string, createdAt: string): PatchTypeWithKey {
  return {
    type: 'NORMALIZED_ENTRY',
    content: {
      entry_type: { type: 'user_message' },
      content: `prompt ${executionId}`,
      timestamp: createdAt,
    },
    patchKey: `${executionId}:user`,
    executionProcessId: executionId,
    processCreatedAt: createdAt,
  };
}

function assistantEntry(executionId: string): PatchTypeWithKey {
  return {
    type: 'NORMALIZED_ENTRY',
    content: {
      entry_type: { type: 'assistant_message' },
      content: 'ok',
      timestamp: null,
    },
    patchKey: `${executionId}:0`,
    executionProcessId: executionId,
  };
}

function comment(
  id: string,
  createdAt: string,
  d: IssueCommentDelivery | null = null
): IssueCommentWithDelivery {
  return {
    comment: {
      id,
      issue_id: 'i1',
      author_id: 'u1',
      parent_id: null,
      message: `message ${id}`,
      created_at: createdAt,
      updated_at: createdAt,
    },
    author_name: 'Ada',
    delivery: d,
    edited_after_delivery: false,
  };
}

function data(
  comments: IssueCommentWithDelivery[],
  deleted: IssueCommentDelivery[] = []
): WorkspaceIssueComments {
  return {
    workspace_id: 'w1',
    issue: {
      id: 'i1',
      simple_id: 'VAS-859',
      title: 'Rust AMT MCP',
      project_id: 'p1',
      issue_url: '/projects/p1/issues/i1',
    },
    comments,
    deleted_deliveries: deleted,
  };
}

describe('mergeIssueComments', () => {
  const entries = [
    userEntry('e1', '2026-10-19T04:00:00Z'),
    assistantEntry('e1'),
    userEntry('e2', '2026-10-19T04:48:00Z'),
    assistantEntry('e2'),
  ];

  it('replaces the delivered turn prompt and places notes between turns', () => {
    const merged = mergeIssueComments(
      entries,
      data([
        comment('note', '2026-10-19T04:30:00Z'),
        comment(
          'reply',
          '2026-10-19T04:45:00Z',
          delivery({
            comment_id: 'reply',
            state: 'delivered',
            execution_process_id: 'e2',
          })
        ),
        comment('late', '2026-10-19T05:00:00Z'),
      ]),
      's1',
      true
    );
    expect(merged.entries.map((e) => e.patchKey)).toEqual([
      'e1:user',
      'e1:0',
      'issue-comment:note',
      'e2:user',
      'e2:0',
      'issue-comment:late',
    ]);
    expect(
      merged.deliveredByExecution.get('e2')?.map((c) => c.entry.comment.id)
    ).toEqual(['reply']);
    // The delivered comment has no second, standalone copy.
    expect(merged.standaloneById.has('reply')).toBe(false);
    expect(standaloneCommentId('issue-comment:note')).toBe('note');
    expect(standaloneCommentId('e1')).toBeNull();
    // Standalone rows never look like user turns.
    const noteRow = merged.entries[2];
    expect(
      noteRow.type === 'NORMALIZED_ENTRY' &&
        noteRow.content.entry_type.type === 'system_message'
    ).toBe(true);
  });

  it('shows comments delivered to another session as standalone rows', () => {
    const merged = mergeIssueComments(
      entries,
      data([
        comment(
          'other',
          '2026-10-19T04:45:00Z',
          delivery({
            comment_id: 'other',
            state: 'delivered',
            session_id: 's2',
            execution_process_id: 'x9',
          })
        ),
      ]),
      's1',
      true
    );
    expect(merged.standaloneById.has('other')).toBe(true);
  });

  it('leaves out comments older than a partially loaded window', () => {
    const merged = mergeIssueComments(
      entries,
      data([comment('old', '2026-10-18T00:00:00Z')]),
      's1',
      false
    );
    expect(merged.standaloneById.size).toBe(0);
    expect(merged.entries).toHaveLength(entries.length);
  });

  it('keeps a delivered-then-deleted comment discoverable on its turn', () => {
    const merged = mergeIssueComments(
      entries,
      data(
        [],
        [
          delivery({
            comment_id: 'gone',
            state: 'delivered',
            execution_process_id: 'e2',
            delivered_message: 'original text',
          }),
        ]
      ),
      's1',
      true
    );
    const carried = merged.deliveredByExecution.get('e2')?.[0];
    expect(carried?.deletedFromIssue).toBe(true);
    expect(carried?.entry.comment.message).toBe('original text');
  });

  it('is a no-op without a linked issue', () => {
    const merged = mergeIssueComments(entries, null, 's1', true);
    expect(merged.entries).toBe(entries);
  });
});
