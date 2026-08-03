import { useCallback, useEffect, useRef, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { X } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { cn } from '../lib/cn';

/** Narrowest the drawer may be dragged. */
export const METRICS_DRAWER_MIN_WIDTH = 360;
/** Widest the drawer may be dragged. */
export const METRICS_DRAWER_MAX_WIDTH = 720;
/** Pixels moved per arrow-key press on the resize handle. */
const RESIZE_KEYBOARD_STEP = 16;

const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

export function clampMetricsDrawerWidth(width: number) {
  if (!Number.isFinite(width)) return METRICS_DRAWER_MIN_WIDTH;
  return Math.min(
    Math.max(Math.round(width), METRICS_DRAWER_MIN_WIDTH),
    METRICS_DRAWER_MAX_WIDTH
  );
}

export interface MetricsDrawerProps {
  open: boolean;
  /** Current width in pixels. Clamped to 360..720 before use. */
  width: number;
  /** Reports a new width during a drag or arrow-key resize. */
  onWidthChange: (width: number) => void;
  onClose: () => void;
  /** Already-translated drawer title. Also labels the dialog. */
  title?: string;
  children?: ReactNode;
}

/**
 * A right-anchored overlay drawer, mirroring `MobileDrawer` (which is
 * left-anchored) — same portal, backdrop and transition.
 *
 * **Props-only.** `@vibe/ui` depends on neither `zustand` nor
 * `@vibe/web-core`, so open/width/selection state must be owned by a
 * container in `@vibe/web-core` and handed down.
 *
 * The drawer *overlays* the app rather than reflowing it (FR-9a), so it is
 * `aria-modal="false"`; it is nevertheless dismissible by `Escape` and by
 * backdrop click, traps Tab while open, and restores focus to whatever was
 * focused before it opened (FR-14).
 */
export function MetricsDrawer({
  open,
  width,
  onWidthChange,
  onClose,
  title,
  children,
}: MetricsDrawerProps) {
  const { t } = useTranslation('common');
  const panelRef = useRef<HTMLDivElement | null>(null);
  const dragRef = useRef<{ startX: number; startWidth: number } | null>(null);

  const clampedWidth = clampMetricsDrawerWidth(width);
  const label =
    title ?? t('metrics.drawerTitle', { defaultValue: 'Server metrics' });

  // Escape closes from anywhere, so the drawer is dismissible by keyboard even
  // when focus has wandered back into the page underneath it.
  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.stopPropagation();
        onClose();
      }
    };
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, [open, onClose]);

  // Move focus into the drawer on open and put it back on close.
  useEffect(() => {
    if (!open) return;
    const previouslyFocused = document.activeElement as HTMLElement | null;
    panelRef.current?.focus();
    return () => {
      if (previouslyFocused && document.contains(previouslyFocused)) {
        previouslyFocused.focus();
      }
    };
  }, [open]);

  // Restore any body styles the drag applied if we unmount mid-drag.
  useEffect(
    () => () => {
      document.body.style.userSelect = '';
      document.body.style.cursor = '';
    },
    []
  );

  const handleTabTrap = useCallback((event: React.KeyboardEvent) => {
    if (event.key !== 'Tab') return;
    const panel = panelRef.current;
    if (!panel) return;
    const focusable = Array.from(
      panel.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)
    ).filter((el) => el.offsetParent !== null || el === document.activeElement);
    if (focusable.length === 0) {
      event.preventDefault();
      panel.focus();
      return;
    }
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    const active = document.activeElement;
    if (event.shiftKey && (active === first || active === panel)) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && active === last) {
      event.preventDefault();
      first.focus();
    }
  }, []);

  const handlePointerDown = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      if (event.button !== 0) return;
      dragRef.current = { startX: event.clientX, startWidth: clampedWidth };
      event.currentTarget.setPointerCapture(event.pointerId);
      document.body.style.userSelect = 'none';
      document.body.style.cursor = 'col-resize';
    },
    [clampedWidth]
  );

  const handlePointerMove = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      const drag = dragRef.current;
      if (!drag) return;
      // Right-anchored: dragging left (smaller clientX) widens the drawer.
      const next = clampMetricsDrawerWidth(
        drag.startWidth + (drag.startX - event.clientX)
      );
      if (next !== clampedWidth) onWidthChange(next);
    },
    [clampedWidth, onWidthChange]
  );

  const endDrag = useCallback((event: React.PointerEvent<HTMLDivElement>) => {
    if (!dragRef.current) return;
    dragRef.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    document.body.style.userSelect = '';
    document.body.style.cursor = '';
  }, []);

  const handleResizeKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      if (event.key === 'ArrowLeft') {
        event.preventDefault();
        onWidthChange(
          clampMetricsDrawerWidth(clampedWidth + RESIZE_KEYBOARD_STEP)
        );
      } else if (event.key === 'ArrowRight') {
        event.preventDefault();
        onWidthChange(
          clampMetricsDrawerWidth(clampedWidth - RESIZE_KEYBOARD_STEP)
        );
      }
    },
    [clampedWidth, onWidthChange]
  );

  return createPortal(
    <>
      {/* Backdrop overlay */}
      <div
        className={cn(
          'fixed inset-0 bg-black/50 z-[100]',
          'transition-opacity duration-200 ease-out',
          open ? 'opacity-100' : 'opacity-0 pointer-events-none'
        )}
        onClick={onClose}
        aria-hidden="true"
      />
      {/* Drawer panel */}
      <div
        ref={panelRef}
        role="dialog"
        // The drawer overlays the app without taking it over (FR-9a).
        aria-modal="false"
        aria-label={label}
        aria-hidden={!open}
        tabIndex={-1}
        onKeyDown={handleTabTrap}
        style={{ width: clampedWidth }}
        className={cn(
          'fixed right-0 top-0 h-full max-w-full bg-primary z-[101]',
          'border-l border-border flex flex-col outline-none',
          'pb-[env(safe-area-inset-bottom)]',
          'transition-transform duration-200 ease-out',
          open ? 'translate-x-0' : 'translate-x-full pointer-events-none'
        )}
      >
        {/* Drag-to-resize handle on the inner edge */}
        <div
          role="separator"
          aria-orientation="vertical"
          aria-label={t('metrics.drawerResize', {
            defaultValue: 'Resize server metrics panel',
          })}
          aria-valuenow={clampedWidth}
          aria-valuemin={METRICS_DRAWER_MIN_WIDTH}
          aria-valuemax={METRICS_DRAWER_MAX_WIDTH}
          tabIndex={0}
          data-testid="metrics-drawer-resize"
          onPointerDown={handlePointerDown}
          onPointerMove={handlePointerMove}
          onPointerUp={endDrag}
          onPointerCancel={endDrag}
          onKeyDown={handleResizeKeyDown}
          className={cn(
            'absolute left-0 top-0 h-full w-base -ml-half z-10',
            'cursor-col-resize touch-none',
            'hover:bg-brand/40 focus:outline-none focus:bg-brand/60'
          )}
        />
        <header className="flex items-center justify-between gap-half p-base border-b border-border shrink-0">
          <h2 className="text-lg text-high truncate">{label}</h2>
          <button
            type="button"
            onClick={onClose}
            aria-label={t('metrics.drawerClose', {
              defaultValue: 'Close server metrics',
            })}
            className="flex items-center justify-center rounded-sm p-half text-low hover:text-normal hover:bg-panel focus:outline-none focus:ring-1 focus:ring-brand"
          >
            <X className="size-icon-base" aria-hidden="true" />
          </button>
        </header>
        {/*
          Both axes are pinned. Per `wiki/mobile-kanban-scrolling.md`, a
          `visible` axis combined with a scrolling one computes to `auto`, so
          `overflow-y-auto` alone would silently create a horizontal scroller.
        */}
        <div className="flex-1 min-h-0 overflow-y-auto overflow-x-hidden p-base">
          {children}
        </div>
      </div>
    </>,
    document.body
  );
}
