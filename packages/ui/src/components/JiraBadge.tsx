import { cn } from '../lib/cn';
import { ArrowSquareOutIcon } from '@phosphor-icons/react';

export interface JiraBadgeProps {
  issueKey: string;
  url: string;
  /** Dimmed when the link is dormant or the issue was deleted in Jira. */
  active?: boolean;
  className?: string;
}

export function JiraBadge({
  issueKey,
  url,
  active = true,
  className,
}: JiraBadgeProps) {
  return (
    <a
      href={url}
      target="_blank"
      rel="noopener noreferrer"
      onClick={(e) => e.stopPropagation()}
      title={
        active ? `Jira ${issueKey}` : `Jira ${issueKey} (no longer syncing)`
      }
      className={cn(
        'flex items-center gap-half px-1.5 py-0.5 rounded text-xs font-medium transition-colors',
        'bg-info/10 text-info hover:bg-info/20',
        !active && 'opacity-50',
        className
      )}
    >
      <ArrowSquareOutIcon className="size-icon-2xs" weight="bold" />
      <span>{issueKey}</span>
    </a>
  );
}
