import { GitBranchIcon } from '@phosphor-icons/react';
import { cn } from '../lib/cn';
import { Tooltip } from './Tooltip';
import {
  SyncErrorIndicator,
  type SyncErrorIndicatorError,
} from './SyncErrorIndicator';
import type { NavbarSectionItem, NavbarActionItem } from './Navbar';

interface WorkspaceRailProps {
  /** Branch / workspace title shown as a header tile tooltip. */
  branch?: string;
  leftItems: NavbarSectionItem[];
  rightItems: NavbarSectionItem[];
  syncErrors?: readonly SyncErrorIndicatorError[] | null;
  onRefreshPage?: () => void;
}

const railTileBaseClassName =
  'flex items-center justify-center w-10 h-10 rounded-lg transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-brand';

function isDivider(
  item: NavbarSectionItem
): item is Extract<NavbarSectionItem, { type: 'divider' }> {
  return item.type === 'divider';
}

function RailDivider() {
  return <div className="my-1 h-px w-6 bg-border" />;
}

function RailActionTile({ item }: { item: NavbarActionItem }) {
  const Icon = item.icon;
  return (
    <Tooltip content={item.tooltip ?? ''} shortcut={item.shortcut} side="right">
      <button
        type="button"
        onClick={item.onClick}
        disabled={item.disabled}
        aria-label={item.tooltip}
        className={cn(
          railTileBaseClassName,
          'cursor-pointer',
          item.isActive
            ? 'bg-brand/20 text-brand hover:bg-brand/20'
            : 'text-low hover:bg-brand/10 hover:text-normal',
          item.disabled && 'cursor-not-allowed opacity-40 hover:bg-transparent'
        )}
      >
        <Icon
          className="size-icon-base"
          weight={item.isActive ? 'fill' : 'regular'}
        />
      </button>
    </Tooltip>
  );
}

function renderItems(items: NavbarSectionItem[], keyPrefix: string) {
  return items.map((item, index) =>
    isDivider(item) ? (
      <RailDivider key={`${keyPrefix}-divider-${index}`} />
    ) : (
      <RailActionTile key={`${keyPrefix}-${item.id}-${index}`} item={item} />
    )
  );
}

/**
 * Vertical workspace toolbar docked to the right edge of the AppBar rail.
 *
 * Holds the per-workspace controls that previously lived in the horizontal
 * top navbar: branch label, sync indicator, and the left/right action groups.
 * Rendered only on routes where the workspace/action providers are available.
 */
export function WorkspaceRail({
  branch,
  leftItems,
  rightItems,
  syncErrors,
  onRefreshPage,
}: WorkspaceRailProps) {
  const hasLeft = leftItems.length > 0;
  const hasRight = rightItems.length > 0;

  return (
    <div
      className={cn(
        'flex flex-col items-center h-full min-h-0 overflow-y-auto p-base gap-1',
        'bg-primary border-r border-border'
      )}
    >
      {branch && (
        <>
          <Tooltip content={branch} side="right">
            <div
              className={cn(railTileBaseClassName, 'text-low')}
              aria-label={branch}
            >
              <GitBranchIcon className="size-icon-base" weight="bold" />
            </div>
          </Tooltip>
          {(hasLeft || hasRight) && <RailDivider />}
        </>
      )}

      {renderItems(leftItems, 'left')}
      {hasLeft && hasRight && <RailDivider />}
      {renderItems(rightItems, 'right')}

      <div className="mt-auto pt-base">
        <SyncErrorIndicator errors={syncErrors} onRefreshPage={onRefreshPage} />
      </div>
    </div>
  );
}
