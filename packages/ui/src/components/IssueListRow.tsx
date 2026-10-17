'use client';

import type { MouseEvent } from 'react';
import { cn } from '../lib/cn';
import { Draggable, type DraggableProvided } from '@hello-pangea/dnd';
import { DotsSixVerticalIcon } from '@phosphor-icons/react';
import { PriorityIcon, type PriorityLevel } from './PriorityIcon';
import { StatusDot } from './StatusDot';
import { KanbanBadge } from './KanbanBadge';
import { KanbanAssignee, type KanbanAssigneeUser } from './KanbanAssignee';
import {
  RelationshipBadge,
  type RelationshipDisplayType,
} from './RelationshipBadge';
import { Checkbox } from './Checkbox';

/**
 * Formats a date as a relative time string (e.g., "1d", "2h", "3m")
 */
function formatRelativeTime(dateString: string): string {
  const date = new Date(dateString);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMinutes = Math.floor(diffMs / (1000 * 60));
  const diffHours = Math.floor(diffMs / (1000 * 60 * 60));
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  if (diffDays > 0) {
    return `${diffDays}d`;
  }
  if (diffHours > 0) {
    return `${diffHours}h`;
  }
  if (diffMinutes > 0) {
    return `${diffMinutes}m`;
  }
  return 'now';
}

const MAX_VISIBLE_TAGS = 2;

export interface IssueListRowIssue {
  id: string;
  simple_id: string;
  title: string;
  priority: PriorityLevel | null;
  created_at: string;
}

export interface IssueListRowTag {
  id: string;
  name: string;
  color: string;
}

export interface IssueListRowRelationship {
  relationshipId: string;
  displayType: RelationshipDisplayType;
  relatedIssueDisplayId: string;
}

export interface IssueListRowProps {
  issue: IssueListRowIssue;
  index: number;
  statusColor: string;
  tags: IssueListRowTag[];
  relationships?: IssueListRowRelationship[];
  assignees: KanbanAssigneeUser[];
  onClick: (e: MouseEvent) => void;
  isSelected: boolean;
  isMultiSelectActive?: boolean;
  isChecked?: boolean;
  onCheckboxChange?: (checked: boolean) => void;
  isMobile?: boolean;
  className?: string;
}

export function IssueListRow({
  issue,
  index,
  statusColor,
  tags,
  relationships = [],
  assignees,
  onClick,
  isSelected,
  isMultiSelectActive = false,
  isChecked = false,
  onCheckboxChange,
  isMobile = false,
  className,
}: IssueListRowProps) {
  const showCheckbox = isMultiSelectActive || isChecked;
  const visibleTags = tags.slice(0, MAX_VISIBLE_TAGS);
  // Mobile only: tags/relationships get their own line; without them the
  // assignee stays on the first line instead of taking a line on its own.
  const hasMetadataLine = visibleTags.length > 0 || relationships.length > 0;

  const renderHandle = (
    dragHandleProps: DraggableProvided['dragHandleProps']
  ) => (
    <div className="relative shrink-0 w-4 flex items-center justify-center">
      {/* Drag handle — hidden when checkbox is shown */}
      <div
        {...dragHandleProps}
        className={cn(
          'cursor-grab',
          showCheckbox ? 'hidden' : 'flex group-hover/row:hidden'
        )}
        onClick={(e) => e.stopPropagation()}
      >
        <DotsSixVerticalIcon className="size-icon-xs text-low" weight="bold" />
      </div>
      {/* Checkbox — shown on hover or when multi-select active */}
      <div
        className={cn(
          'items-center justify-center',
          showCheckbox ? 'flex' : 'hidden group-hover/row:flex'
        )}
        onClick={(e) => e.stopPropagation()}
      >
        <Checkbox
          checked={isChecked}
          onCheckedChange={(checked) => {
            onCheckboxChange?.(checked);
          }}
        />
      </div>
    </div>
  );

  const metadata = (
    <>
      {visibleTags.length > 0 && (
        <div className="flex items-center gap-half">
          {visibleTags.map((tag) => (
            <KanbanBadge key={tag.id} name={tag.name} color={tag.color} />
          ))}
        </div>
      )}
      {relationships.length > 0 && (
        <div className="flex items-center gap-half">
          {relationships.slice(0, 2).map((rel) => (
            <RelationshipBadge
              key={rel.relationshipId}
              displayType={rel.displayType}
              relatedIssueDisplayId={rel.relatedIssueDisplayId}
              compact
            />
          ))}
          {relationships.length > 2 && (
            <span className="text-sm text-low">
              +{relationships.length - 2}
            </span>
          )}
        </div>
      )}
    </>
  );

  return (
    <Draggable draggableId={issue.id} index={index}>
      {(provided, snapshot) => (
        <div
          ref={provided.innerRef}
          {...provided.draggableProps}
          role="button"
          tabIndex={0}
          onClick={onClick}
          onKeyDown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault();
              onClick(e as unknown as MouseEvent);
            }
          }}
          className={cn(
            'group/row flex transition-colors',
            isMobile
              ? 'flex-col gap-half px-base py-half'
              : 'items-center justify-between gap-double px-double py-half',
            'hover:bg-secondary',
            (isSelected || isChecked) && 'bg-secondary',
            snapshot.isDragging && 'bg-secondary shadow-lg cursor-grabbing',
            className
          )}
        >
          {isMobile ? (
            <>
              {/* Line 1: handle, priority, ID, status, title, age */}
              <div className="flex items-center gap-base min-w-0">
                {renderHandle(provided.dragHandleProps)}
                <PriorityIcon priority={issue.priority} />
                <span className="font-ibm-plex-mono text-sm text-normal shrink-0">
                  {issue.simple_id}
                </span>
                <StatusDot color={statusColor} />
                <span className="flex-1 min-w-0 text-base text-high truncate">
                  {issue.title}
                </span>
                {!hasMetadataLine && (
                  <KanbanAssignee assignees={assignees} className="shrink-0" />
                )}
                <span className="text-sm text-low shrink-0">
                  {formatRelativeTime(issue.created_at)}
                </span>
              </div>

              {/* Line 2: tags, relationships, assignee — clipped, never wraps */}
              {hasMetadataLine && (
                <div className="flex items-center gap-half min-w-0 pl-6">
                  <div className="flex flex-1 items-center gap-half min-w-0 overflow-hidden">
                    {metadata}
                  </div>
                  <KanbanAssignee assignees={assignees} className="shrink-0" />
                </div>
              )}
            </>
          ) : (
            <>
              {/* Left side: Checkbox/Drag handle, Priority, ID, Status, Title */}
              <div className="flex items-center gap-double flex-1 min-w-0">
                {renderHandle(provided.dragHandleProps)}
                <PriorityIcon priority={issue.priority} />
                <span className="font-ibm-plex-mono text-sm text-normal shrink-0">
                  {issue.simple_id}
                </span>
                <StatusDot color={statusColor} />
                <span className="text-base text-high truncate">
                  {issue.title}
                </span>
              </div>

              {/* Right side: Tags, Assignee, Age */}
              <div className="flex items-center gap-base shrink-0">
                {metadata}
                <KanbanAssignee assignees={assignees} />
                <span className="text-sm text-low w-5 text-right">
                  {formatRelativeTime(issue.created_at)}
                </span>
              </div>
            </>
          )}
        </div>
      )}
    </Draggable>
  );
}
