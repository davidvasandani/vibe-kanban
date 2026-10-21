import { createContext, useContext } from 'react';
import type { LinkedIssueSummary } from 'shared/types';
import type { ConversationIssueComment } from '@/shared/lib/issueCommentDelivery';

/**
 * Linked issue comments merged into the current conversation
 * (`mergeIssueComments`), for the entry renderer.
 */
export interface IssueCommentsContextValue {
  readonly issue: LinkedIssueSummary | null;
  readonly deliveredByExecution: ReadonlyMap<
    string,
    readonly ConversationIssueComment[]
  >;
  readonly standaloneById: ReadonlyMap<string, ConversationIssueComment>;
}

const EMPTY: IssueCommentsContextValue = {
  issue: null,
  deliveredByExecution: new Map(),
  standaloneById: new Map(),
};

export const IssueCommentsContext =
  createContext<IssueCommentsContextValue>(EMPTY);

export function useIssueCommentsInConversation(): IssueCommentsContextValue {
  return useContext(IssueCommentsContext);
}
