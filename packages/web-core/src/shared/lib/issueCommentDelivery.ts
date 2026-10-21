import type {
  DeliveryTargetCandidate,
  IssueAgentTargets,
  IssueCommentDelivery,
  IssueCommentWithDelivery,
  NormalizedEntry,
  WorkspaceIssueComments,
} from 'shared/types';
import type { PatchTypeWithKey } from '@/shared/hooks/useConversationHistory/types';

/**
 * Issue comment ↔ agent session delivery, as presented in the issue panel and
 * the workspace conversation. The backend owns delivery; these helpers only
 * describe its persisted state. Saving a comment is never shown as delivery.
 */

export type CommentIntent = 'note' | 'reply';

/** Reply is the default only when one waiting/running session would get it. */
export function defaultCommentIntent(
  targets: IssueAgentTargets | null | undefined
): CommentIntent {
  return targets?.default_session_id ? 'reply' : 'note';
}

export function candidateLabel(candidate: DeliveryTargetCandidate): string {
  const workspace = candidate.workspace_name || candidate.workspace_branch;
  return candidate.session_name
    ? `${workspace} · ${candidate.session_name}`
    : workspace;
}

export type DeliveryTone = 'neutral' | 'progress' | 'success' | 'warning';

export type DeliveryAction = 'retry' | 'choose_target' | 'send';

/** What a comment's delivery chip shows. `labelKey` is under `kanban.delivery`. */
export interface DeliveryChipModel {
  readonly tone: DeliveryTone;
  readonly labelKey: string;
  readonly params: Record<string, string>;
  /** Backend diagnostic, shown verbatim (constitution XI). */
  readonly detail: string | null;
  readonly actions: readonly DeliveryAction[];
  /** Session that received or will receive the comment. */
  readonly sessionId: string | null;
  readonly workspaceId: string | null;
  readonly editedAfterDelivery: boolean;
}

interface DescribeInput {
  readonly delivery: IssueCommentDelivery | null | undefined;
  readonly editedAfterDelivery?: boolean;
  /** A delivery request from this client failed before the backend stored it. */
  readonly requestError?: string | null;
  readonly targets: IssueAgentTargets | null | undefined;
}

function sessionName(
  targets: IssueAgentTargets | null | undefined,
  sessionId: string | null
): string {
  const candidate = targets?.candidates.find(
    (item) => item.session_id === sessionId
  );
  return candidate ? candidateLabel(candidate) : (sessionId?.slice(0, 8) ?? '');
}

/** `null` when there is nothing to say (no delivery and nothing linked). */
export function describeDelivery({
  delivery,
  editedAfterDelivery = false,
  requestError,
  targets,
}: DescribeInput): DeliveryChipModel | null {
  const canChoose = (targets?.candidates.length ?? 0) > 0;
  const base = {
    params: {} as Record<string, string>,
    detail: null as string | null,
    sessionId: delivery?.session_id ?? null,
    workspaceId: delivery?.workspace_id ?? null,
    editedAfterDelivery,
  };

  if (!delivery) {
    if (requestError) {
      return {
        ...base,
        tone: 'warning',
        labelKey: 'requestFailed',
        detail: requestError,
        actions: ['retry'],
      };
    }
    if (!targets || targets.candidates.length === 0 || !targets.issue_active) {
      return null;
    }
    return {
      ...base,
      tone: 'neutral',
      labelKey: 'notSent',
      actions: ['send'],
    };
  }

  const name = sessionName(targets, delivery.session_id);
  switch (delivery.state) {
    case 'delivered':
      return {
        ...base,
        tone: 'success',
        labelKey: 'delivered',
        params: { session: name },
        actions: [],
      };
    case 'pending':
      return {
        ...base,
        tone: 'progress',
        labelKey:
          delivery.reason === 'retry_scheduled'
            ? 'retrying'
            : delivery.reason === 'queued_message'
              ? 'queuedBehindMessage'
              : 'queued',
        params: { session: name },
        detail: delivery.error,
        actions: [],
      };
    case 'dispatching':
      return {
        ...base,
        tone: 'progress',
        labelKey: 'dispatching',
        params: { session: name },
        actions: [],
      };
    case 'needs_target':
      return {
        ...base,
        tone: 'warning',
        labelKey: 'needsTarget',
        actions: canChoose ? ['choose_target'] : [],
      };
    case 'no_target':
      return {
        ...base,
        tone: 'warning',
        labelKey: 'noTarget',
        params: { reason: delivery.reason ?? 'no_sessions' },
        actions: canChoose ? ['retry', 'choose_target'] : ['retry'],
      };
    case 'failed':
      return {
        ...base,
        tone: 'warning',
        labelKey: 'failed',
        params: { reason: delivery.reason ?? 'start_failed' },
        detail: delivery.error,
        actions: canChoose ? ['retry', 'choose_target'] : ['retry'],
      };
    case 'blocked':
      return {
        ...base,
        tone: 'neutral',
        labelKey: 'blocked',
        params: { reason: delivery.reason ?? 'loop_guard' },
        actions: [],
      };
    case 'cancelled':
      return {
        ...base,
        tone: 'neutral',
        labelKey: 'cancelled',
        actions: [],
      };
  }
}

/** Polled reads carry their own deadline (constitution XLIII). */
export function withDeadline<T>(
  operation: Promise<T>,
  signal: AbortSignal | undefined,
  ms = 10_000
): Promise<T> {
  const deadline = AbortSignal.timeout(ms);
  const combined = signal ? AbortSignal.any([signal, deadline]) : deadline;
  return new Promise((resolve, reject) => {
    const abort = () =>
      reject(combined.reason ?? new Error('Request deadline reached'));
    if (combined.aborted) return abort();
    combined.addEventListener('abort', abort, { once: true });
    operation.then(
      (value) => {
        combined.removeEventListener('abort', abort);
        resolve(value);
      },
      (error) => {
        combined.removeEventListener('abort', abort);
        reject(error);
      }
    );
  });
}

// ---------------------------------------------------------------------------
// Conversation merge
// ---------------------------------------------------------------------------

/** Execution-process id prefix of synthetic issue-comment rows. */
export const ISSUE_COMMENT_ROW_PREFIX = 'issue-comment:';

export interface ConversationIssueComment {
  readonly entry: IssueCommentWithDelivery;
  /** The comment was deleted from the issue after it was delivered. */
  readonly deletedFromIssue: boolean;
}

export interface MergedIssueComments {
  readonly entries: PatchTypeWithKey[];
  /** Comments carried by a turn's prompt, keyed by that execution id. */
  readonly deliveredByExecution: ReadonlyMap<
    string,
    readonly ConversationIssueComment[]
  >;
  /** Standalone comment rows, keyed by comment id. */
  readonly standaloneById: ReadonlyMap<string, ConversationIssueComment>;
}

function userEntryProcessId(entry: PatchTypeWithKey): string | null {
  return entry.patchKey.endsWith(':user') ? entry.executionProcessId : null;
}

/**
 * Show the linked issue's comments in a session's conversation.
 *
 * - A comment delivered to this session is rendered in place of the user
 *   message of the turn it started (no duplicate copy).
 * - Every other comment becomes its own row, placed before the first turn
 *   that started after it, so chronology holds between turns. Comments
 *   older than the loaded window are left out until that history loads
 *   (`historyComplete`), rather than being pinned to the top.
 */
export function mergeIssueComments(
  entries: PatchTypeWithKey[],
  data: WorkspaceIssueComments | null | undefined,
  sessionId: string | null | undefined,
  historyComplete: boolean
): MergedIssueComments {
  const deliveredByExecution = new Map<string, ConversationIssueComment[]>();
  const standaloneById = new Map<string, ConversationIssueComment>();
  if (!data?.issue) {
    return { entries, deliveredByExecution, standaloneById };
  }

  const turnIds = new Set(
    entries.map(userEntryProcessId).filter((id): id is string => id !== null)
  );

  const deleted: IssueCommentWithDelivery[] = data.deleted_deliveries.map(
    (delivery) => ({
      comment: {
        id: delivery.comment_id,
        issue_id: delivery.issue_id,
        author_id: delivery.comment_author_id,
        parent_id: null,
        message: delivery.delivered_message ?? '',
        created_at: delivery.comment_created_at,
        updated_at: delivery.comment_created_at,
      },
      author_name: null,
      delivery,
      edited_after_delivery: false,
    })
  );

  const standalone: ConversationIssueComment[] = [];
  for (const [entry, deletedFromIssue] of [
    ...data.comments.map((item) => [item, false] as const),
    ...deleted.map((item) => [item, true] as const),
  ]) {
    const delivery = entry.delivery;
    const execution = delivery?.execution_process_id ?? null;
    if (
      delivery?.state === 'delivered' &&
      delivery.session_id === sessionId &&
      execution
    ) {
      if (turnIds.has(execution)) {
        const list = deliveredByExecution.get(execution) ?? [];
        list.push({ entry, deletedFromIssue });
        deliveredByExecution.set(execution, list);
      }
      continue;
    }
    if (deletedFromIssue) continue;
    standalone.push({ entry, deletedFromIssue });
  }

  standalone.sort((a, b) =>
    a.entry.comment.created_at.localeCompare(b.entry.comment.created_at)
  );

  const firstTurnAt = entries.find(userEntryProcessId)?.processCreatedAt;
  const result: PatchTypeWithKey[] = [];
  let next = 0;
  const emitUntil = (before: string | undefined) => {
    while (
      next < standalone.length &&
      (before === undefined ||
        standalone[next].entry.comment.created_at < before)
    ) {
      const item = standalone[next++];
      const comment = item.entry.comment;
      if (
        !historyComplete &&
        firstTurnAt !== undefined &&
        comment.created_at < firstTurnAt
      ) {
        continue;
      }
      standaloneById.set(comment.id, item);
      const content: NormalizedEntry = {
        // A system row, not a user message: it must not split agent turns.
        entry_type: { type: 'system_message' },
        content: comment.message,
        timestamp: comment.created_at,
      };
      result.push({
        type: 'NORMALIZED_ENTRY',
        content,
        patchKey: `${ISSUE_COMMENT_ROW_PREFIX}${comment.id}`,
        executionProcessId: `${ISSUE_COMMENT_ROW_PREFIX}${comment.id}`,
        processCreatedAt: comment.created_at,
      });
    }
  };

  for (const entry of entries) {
    if (userEntryProcessId(entry)) {
      emitUntil(entry.processCreatedAt);
    }
    result.push(entry);
  }
  emitUntil(undefined);

  return { entries: result, deliveredByExecution, standaloneById };
}

export function standaloneCommentId(executionProcessId: string | undefined) {
  return executionProcessId?.startsWith(ISSUE_COMMENT_ROW_PREFIX)
    ? executionProcessId.slice(ISSUE_COMMENT_ROW_PREFIX.length)
    : null;
}
