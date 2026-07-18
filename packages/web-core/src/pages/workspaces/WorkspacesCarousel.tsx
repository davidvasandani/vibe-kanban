import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { PointerEvent as ReactPointerEvent } from 'react';
import { useTranslation } from 'react-i18next';
import {
  ArrowLeftIcon,
  HandIcon,
  SortAscendingIcon,
  SpinnerIcon,
} from '@phosphor-icons/react';
import { useWorkspaceContext } from '@/shared/hooks/useWorkspaceContext';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import {
  useUiPreferencesStore,
  type CarouselSortMode,
} from '@/shared/stores/useUiPreferencesStore';
import { PropertyDropdown } from '@vibe/ui/components/PropertyDropdown';
import { IconButton } from '@vibe/ui/components/IconButton';
import { needsFeedback, sortForCarousel } from './carousel/carouselSort';
import { WorkspaceCarouselColumn } from './carousel/WorkspaceCarouselColumn';

const COLUMN_WIDTH = 420;
const LIVE_WINDOW_PAD = 2;
const MAX_LIVE_COLUMNS = 8;
const MAX_STICKY_LIVE_COLUMNS = 4;
const RESORT_DEBOUNCE_MS = 1000;
const BLUR_APPLY_DELAY_MS = 150;

const SORT_MODES: CarouselSortMode[] = [
  'needs_feedback',
  'updated_at',
  'created_at',
  'name',
];

function arraysEqual(a: string[], b: string[]): boolean {
  return a.length === b.length && a.every((value, index) => value === b[index]);
}

export function WorkspacesCarousel() {
  const { t } = useTranslation('common');
  const appNavigation = useAppNavigation();
  const { activeWorkspaces, isWorkspacesListLoading } = useWorkspaceContext();
  const carouselSort = useUiPreferencesStore((s) => s.carouselSort);
  const setCarouselSort = useUiPreferencesStore((s) => s.setCarouselSort);

  const workspacesById = useMemo(
    () => new Map(activeWorkspaces.map((ws) => [ws.id, ws])),
    [activeWorkspaces]
  );

  const targetOrder = useMemo(
    () => sortForCarousel(activeWorkspaces, carouselSort).map((ws) => ws.id),
    [activeWorkspaces, carouselSort]
  );
  const targetOrderRef = useRef(targetOrder);
  targetOrderRef.current = targetOrder;

  // Rendered order lags the target order: status changes re-sort through a
  // debounce and never while focus is inside a column, so columns don't jump
  // out from under the operator mid-interaction (spec FR-7).
  const [appliedOrder, setAppliedOrder] = useState<string[] | null>(null);
  const appliedOrderRef = useRef<string[] | null>(null);
  appliedOrderRef.current = appliedOrder;

  const focusedIdsRef = useRef(new Set<string>());
  const pendingApplyRef = useRef(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Columns the user has interacted with stay live even when scrolled out of
  // the mount window, so their chat drafts are not dropped by unmounting.
  const stickyLiveRef = useRef<string[]>([]);

  const applyTargetOrder = useCallback(() => {
    pendingApplyRef.current = false;
    setAppliedOrder(targetOrderRef.current);
  }, []);

  useEffect(() => {
    if (appliedOrderRef.current === null) {
      setAppliedOrder(targetOrder);
      return;
    }
    // An already-scheduled timer stays alive across unrelated stream updates
    // (it applies the latest target via the ref); resetting it here would let
    // frequent summary polls starve the re-sort forever.
    if (arraysEqual(appliedOrderRef.current, targetOrder)) return;
    if (debounceRef.current) return;
    debounceRef.current = setTimeout(() => {
      debounceRef.current = null;
      if (focusedIdsRef.current.size === 0) {
        applyTargetOrder();
      } else {
        pendingApplyRef.current = true;
      }
    }, RESORT_DEBOUNCE_MS);
  }, [targetOrder, applyTargetOrder]);

  useEffect(
    () => () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    },
    []
  );

  // A column that unmounts while focused (workspace archived/deleted) never
  // fires blur; prune stale focus state so ordering doesn't freeze forever.
  useEffect(() => {
    let removed = false;
    for (const id of [...focusedIdsRef.current]) {
      if (!workspacesById.has(id)) {
        focusedIdsRef.current.delete(id);
        removed = true;
      }
    }
    stickyLiveRef.current = stickyLiveRef.current.filter((id) =>
      workspacesById.has(id)
    );
    if (
      removed &&
      focusedIdsRef.current.size === 0 &&
      pendingApplyRef.current
    ) {
      applyTargetOrder();
    }
  }, [workspacesById, applyTargetOrder]);

  const handleSortChange = useCallback(
    (mode: CarouselSortMode) => {
      setCarouselSort(mode);
      // A user-initiated sort change applies immediately.
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
        debounceRef.current = null;
      }
      pendingApplyRef.current = false;
      setAppliedOrder(null);
    },
    [setCarouselSort]
  );

  const handleChatFocusChange = useCallback(
    (workspaceId: string, focused: boolean) => {
      if (focused) {
        // Interaction engages exactly one column at a time.
        focusedIdsRef.current.clear();
        focusedIdsRef.current.add(workspaceId);
        stickyLiveRef.current = [
          workspaceId,
          ...stickyLiveRef.current.filter((id) => id !== workspaceId),
        ].slice(0, MAX_STICKY_LIVE_COLUMNS);
      } else {
        focusedIdsRef.current.delete(workspaceId);
        if (focusedIdsRef.current.size === 0 && pendingApplyRef.current) {
          // Delay slightly so an interaction moving to another column lands
          // first.
          setTimeout(() => {
            if (focusedIdsRef.current.size === 0 && pendingApplyRef.current) {
              applyTargetOrder();
            }
          }, BLUR_APPLY_DELAY_MS);
        }
      }
    },
    [applyTargetOrder]
  );

  // Interacting anywhere outside a column (toolbar, empty strip) releases the
  // order freeze — blur alone can't cover this because chat editors autofocus
  // and focus may never have been inside the engaged column.
  const handleRootPointerDownCapture = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      const target = event.target as HTMLElement | null;
      if (target?.closest?.('[data-carousel-column]')) return;
      if (focusedIdsRef.current.size === 0) return;
      focusedIdsRef.current.clear();
      if (pendingApplyRef.current) {
        setTimeout(() => {
          if (focusedIdsRef.current.size === 0 && pendingApplyRef.current) {
            applyTargetOrder();
          }
        }, BLUR_APPLY_DELAY_MS);
      }
    },
    [applyTargetOrder]
  );

  const renderedWorkspaces = useMemo(() => {
    const applied = appliedOrder ?? targetOrder;
    const existing = applied.filter((id) => workspacesById.has(id));
    const known = new Set(existing);
    // New workspaces append on the right immediately; they slot into place on
    // the next applied re-sort without disturbing the current order.
    const newcomers = targetOrder.filter((id) => !known.has(id));
    return [...existing, ...newcomers].map((id) => workspacesById.get(id)!);
  }, [appliedOrder, targetOrder, workspacesById]);

  // Mount windowing: only columns near the viewport mount live chat streams.
  const stripRef = useRef<HTMLDivElement>(null);
  const [scrollState, setScrollState] = useState({ scrollLeft: 0, width: 0 });
  const scrollRafRef = useRef<number | null>(null);

  const measureStrip = useCallback(() => {
    const strip = stripRef.current;
    if (!strip) return;
    setScrollState((prev) => {
      const next = { scrollLeft: strip.scrollLeft, width: strip.clientWidth };
      return prev.scrollLeft === next.scrollLeft && prev.width === next.width
        ? prev
        : next;
    });
  }, []);

  useEffect(() => {
    measureStrip();
    window.addEventListener('resize', measureStrip);
    return () => {
      window.removeEventListener('resize', measureStrip);
      if (scrollRafRef.current !== null) {
        cancelAnimationFrame(scrollRafRef.current);
      }
    };
  }, [measureStrip]);

  const handleStripScroll = useCallback(() => {
    if (scrollRafRef.current !== null) return;
    scrollRafRef.current = requestAnimationFrame(() => {
      scrollRafRef.current = null;
      measureStrip();
    });
  }, [measureStrip]);

  const liveIds = useMemo(() => {
    const start = Math.max(
      0,
      Math.floor(scrollState.scrollLeft / COLUMN_WIDTH) - LIVE_WINDOW_PAD
    );
    const visibleCount = Math.ceil(
      Math.max(scrollState.width, COLUMN_WIDTH) / COLUMN_WIDTH
    );
    const windowSize = Math.min(
      visibleCount + LIVE_WINDOW_PAD * 2,
      MAX_LIVE_COLUMNS
    );
    const live = new Set(
      renderedWorkspaces.slice(start, start + windowSize).map((ws) => ws.id)
    );
    for (const id of stickyLiveRef.current) {
      if (workspacesById.has(id)) live.add(id);
    }
    return live;
  }, [scrollState, renderedWorkspaces, workspacesById]);

  const feedbackCount = useMemo(
    () => activeWorkspaces.filter((ws) => needsFeedback(ws)).length,
    [activeWorkspaces]
  );

  const sortOptions = useMemo(
    () =>
      SORT_MODES.map((mode) => ({
        value: mode,
        label: t(`workspaces.carousel.sort.${mode}`),
      })),
    [t]
  );

  return (
    <div
      className="flex h-full min-h-0 flex-col bg-primary"
      onPointerDownCapture={handleRootPointerDownCapture}
    >
      {/* Toolbar */}
      <div className="flex shrink-0 items-center gap-base border-b border-border bg-secondary px-base py-half">
        <IconButton
          icon={ArrowLeftIcon}
          onClick={() => appNavigation.goToWorkspaces()}
          aria-label={t('workspaces.carousel.backToWorkspaces')}
          title={t('workspaces.carousel.backToWorkspaces')}
        />
        <h1 className="text-sm font-medium text-normal">
          {t('workspaces.carousel.title')}
        </h1>
        <span className="flex items-center gap-half text-xs text-low">
          <HandIcon
            className={feedbackCount > 0 ? 'text-brand' : undefined}
            weight="fill"
          />
          {t('workspaces.carousel.needsFeedbackCount', {
            count: feedbackCount,
          })}
        </span>
        <div className="ml-auto">
          <PropertyDropdown
            value={carouselSort}
            options={sortOptions}
            onChange={handleSortChange}
            icon={SortAscendingIcon}
          />
        </div>
      </div>

      {/* Column strip: the only horizontal scroller; columns own vertical. */}
      {isWorkspacesListLoading ? (
        <div className="flex flex-1 items-center justify-center">
          <SpinnerIcon className="size-6 animate-spin text-low" />
        </div>
      ) : renderedWorkspaces.length === 0 ? (
        <div className="flex flex-1 items-center justify-center">
          <p className="text-low">{t('workspaces.carousel.empty')}</p>
        </div>
      ) : (
        <div
          ref={stripRef}
          onScroll={handleStripScroll}
          className="flex min-h-0 flex-1 overflow-x-auto overflow-y-hidden"
        >
          {renderedWorkspaces.map((workspace) => (
            <WorkspaceCarouselColumn
              key={workspace.id}
              workspace={workspace}
              live={liveIds.has(workspace.id)}
              onChatFocusChange={handleChatFocusChange}
            />
          ))}
        </div>
      )}
    </div>
  );
}
