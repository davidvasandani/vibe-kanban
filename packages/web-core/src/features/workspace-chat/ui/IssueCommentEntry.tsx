import { useTranslation } from 'react-i18next';
import { IssueCommentConversationCard } from '@vibe/ui/components/IssueCommentDelivery';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import type { ConversationIssueComment } from '@/shared/lib/issueCommentDelivery';
import { formatConversationTimestamp } from './ConversationTimestamp';
import { useIssueCommentsInConversation } from '../model/contexts/IssueCommentsContext';

interface Props {
  readonly item: ConversationIssueComment;
  readonly renderMarkdown: (content: string) => React.ReactNode;
  /** True when this comment's text is what the agent received this turn. */
  readonly deliveredHere: boolean;
}

/** A linked issue comment in the conversation, with a route back to it. */
export function IssueCommentEntry({
  item,
  renderMarkdown,
  deliveredHere,
}: Props) {
  const { t } = useTranslation('common');
  const appNavigation = useAppNavigation();
  const { issue } = useIssueCommentsInConversation();
  const { comment, delivery } = item.entry;
  // The agent received the snapshot; show that text on the turn it started.
  const message =
    deliveredHere && delivery?.delivered_message
      ? delivery.delivered_message
      : comment.message;
  const time = formatConversationTimestamp(comment.created_at);

  const notices: string[] = [];
  if (item.deletedFromIssue)
    notices.push(t('kanban.delivery.deletedFromIssue'));
  if (deliveredHere && item.entry.edited_after_delivery) {
    notices.push(t('kanban.delivery.editedAfterDelivery'));
  }
  let status: string | null = null;
  if (deliveredHere) {
    status = t('kanban.delivery.resumedThisSession');
  } else if (!delivery) {
    status = t('kanban.delivery.notSentShort');
  } else if (delivery.state === 'delivered') {
    status = t('kanban.delivery.deliveredElsewhere');
  } else if (delivery.state === 'pending' || delivery.state === 'dispatching') {
    status = t('kanban.delivery.queuedShort');
  } else {
    status = t(`kanban.delivery.stateShort.${delivery.state}`);
  }

  return (
    <IssueCommentConversationCard
      indicatorLabel={t('kanban.delivery.indicator', {
        issue: issue?.simple_id ?? '',
      })}
      authorName={item.entry.author_name ?? t('kanban.unknownUser')}
      timeLabel={
        time ? (
          <time
            dateTime={time.dateTime}
            title={time.fullLabel}
            aria-label={time.fullLabel}
          >
            {time.compactLabel}
          </time>
        ) : null
      }
      message={renderMarkdown(message)}
      notices={notices}
      status={<span className="text-xs text-low">{status}</span>}
      openIssue={
        issue
          ? {
              label: t('kanban.delivery.openOnIssue'),
              onClick: () =>
                appNavigation.goToProjectIssue(issue.project_id, issue.id),
            }
          : null
      }
    />
  );
}
