import type { ReactNode } from 'react';
import {
  ArrowClockwiseIcon,
  ArrowSquareOutIcon,
  ChatCircleTextIcon,
  CheckCircleIcon,
  ClockIcon,
  RobotIcon,
  WarningIcon,
} from '@phosphor-icons/react';
import { cn } from '../lib/cn';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from './Dropdown';

/**
 * Presentational pieces for delivering issue comments to agent sessions.
 * Containers in web-core supply the state and every label.
 */

export type CommentIntentValue = 'note' | 'reply';

export interface IssueCommentIntentToggleProps {
  value: CommentIntentValue;
  onChange: (value: CommentIntentValue) => void;
  noteLabel: string;
  replyLabel: string;
  /** Explains what the selected option will do (target, or why not). */
  hint?: string;
  replyDisabled?: boolean;
  replyDisabledReason?: string;
}

/** "Add a note" versus "Reply to agent", chosen before posting. */
export function IssueCommentIntentToggle({
  value,
  onChange,
  noteLabel,
  replyLabel,
  hint,
  replyDisabled,
  replyDisabledReason,
}: IssueCommentIntentToggleProps) {
  const option = (
    optionValue: CommentIntentValue,
    label: string,
    disabled?: boolean,
    title?: string
  ) => (
    <button
      type="button"
      role="radio"
      aria-checked={value === optionValue}
      disabled={disabled}
      title={title}
      onClick={() => onChange(optionValue)}
      className={cn(
        'px-base py-half rounded-sm text-xs transition-colors',
        value === optionValue
          ? 'bg-panel border border-border text-high'
          : 'border border-transparent text-low hover:text-normal',
        'disabled:opacity-50 disabled:cursor-not-allowed'
      )}
    >
      {label}
    </button>
  );

  return (
    <div className="flex flex-col gap-half min-w-0">
      <div role="radiogroup" className="flex items-center gap-half">
        {option('note', noteLabel)}
        {option('reply', replyLabel, replyDisabled, replyDisabledReason)}
      </div>
      {hint && <span className="text-xs text-low truncate">{hint}</span>}
    </div>
  );
}

export type DeliveryChipTone = 'neutral' | 'progress' | 'success' | 'warning';

export interface DeliveryChipAction {
  key: string;
  label: string;
  onClick: () => void;
}

export interface DeliveryChipTargetChoice {
  label: string;
  options: { id: string; label: string }[];
  onSelect: (id: string) => void;
}

export interface IssueCommentDeliveryChipProps {
  tone: DeliveryChipTone;
  label: string;
  /** Backend diagnostic shown verbatim. */
  detail?: string | null;
  /** Extra notice, e.g. "edited after delivery". */
  notice?: string | null;
  actions?: DeliveryChipAction[];
  targetChoice?: DeliveryChipTargetChoice | null;
  link?: { label: string; onClick: () => void } | null;
}

const TONE_ICON = {
  neutral: ChatCircleTextIcon,
  progress: ClockIcon,
  success: CheckCircleIcon,
  warning: WarningIcon,
} as const;

/** One line stating where a comment went, with the actions it allows. */
export function IssueCommentDeliveryChip({
  tone,
  label,
  detail,
  notice,
  actions = [],
  targetChoice,
  link,
}: IssueCommentDeliveryChipProps) {
  const Icon = TONE_ICON[tone];
  return (
    <div
      className="flex flex-col gap-half text-xs"
      data-testid="issue-comment-delivery"
      data-tone={tone}
    >
      <div className="flex items-center gap-base flex-wrap">
        <span
          className={cn(
            'flex items-center gap-half',
            tone === 'success' && 'text-success',
            tone === 'warning' && 'text-warning',
            (tone === 'neutral' || tone === 'progress') && 'text-low'
          )}
        >
          <Icon size={12} />
          {label}
        </span>
        {link && (
          <button
            type="button"
            onClick={link.onClick}
            className="flex items-center gap-half text-low hover:text-normal"
          >
            <ArrowSquareOutIcon size={12} />
            {link.label}
          </button>
        )}
        {actions.map((action) => (
          <button
            key={action.key}
            type="button"
            onClick={action.onClick}
            className="flex items-center gap-half text-low hover:text-normal"
          >
            <ArrowClockwiseIcon size={12} />
            {action.label}
          </button>
        ))}
        {targetChoice && targetChoice.options.length > 0 && (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <button
                type="button"
                className="flex items-center gap-half text-low hover:text-normal"
              >
                <RobotIcon size={12} />
                {targetChoice.label}
              </button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start">
              {targetChoice.options.map((option) => (
                <DropdownMenuItem
                  key={option.id}
                  onSelect={() => targetChoice.onSelect(option.id)}
                >
                  {option.label}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        )}
      </div>
      {notice && <span className="text-low">{notice}</span>}
      {detail && (
        <span className="text-low whitespace-pre-wrap break-words">
          {detail}
        </span>
      )}
    </div>
  );
}

export interface IssueCommentConversationCardProps {
  /** e.g. "Issue comment · VAS-859". */
  indicatorLabel: string;
  authorName: string;
  /** Formatted time of the comment itself (not of the turn). */
  timeLabel: ReactNode;
  message: ReactNode;
  /** Short notices: "edited after delivery", "deleted from issue". */
  notices?: string[];
  status?: ReactNode;
  openIssue?: { label: string; onClick: () => void } | null;
}

/** An issue comment shown in a workspace conversation. */
export function IssueCommentConversationCard({
  indicatorLabel,
  authorName,
  timeLabel,
  message,
  notices = [],
  status,
  openIssue,
}: IssueCommentConversationCardProps) {
  return (
    <div
      className="flex flex-col gap-half rounded-sm border border-border bg-secondary p-double"
      data-testid="issue-comment-card"
    >
      <div className="flex items-center gap-base text-xs text-low flex-wrap">
        <span className="flex items-center gap-half text-normal">
          <ChatCircleTextIcon size={14} />
          {indicatorLabel}
        </span>
        <span>·</span>
        <span className="font-medium">{authorName}</span>
        <span>·</span>
        {timeLabel}
        {openIssue && (
          <button
            type="button"
            onClick={openIssue.onClick}
            className="ml-auto flex items-center gap-half hover:text-normal"
          >
            <ArrowSquareOutIcon size={12} />
            {openIssue.label}
          </button>
        )}
      </div>
      <div className="min-w-0">{message}</div>
      {notices.map((notice) => (
        <span key={notice} className="text-xs text-low">
          {notice}
        </span>
      ))}
      {status}
    </div>
  );
}
