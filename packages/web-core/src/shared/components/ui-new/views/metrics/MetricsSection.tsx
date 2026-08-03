import type { ReactNode } from 'react';
import { CaretDownIcon } from '@phosphor-icons/react';

import { cn } from '@/shared/lib/utils';

export interface MetricsSectionProps {
  /** Panel id, e.g. `'cpu'`. Also the `expandedPanels` key in the store. */
  panelId: string;
  /** Already-translated section title. */
  title: string;
  /** Already-translated summary rendered on the header row. */
  summary?: ReactNode;
  expanded: boolean;
  onToggle: (panelId: string) => void;
  /**
   * The node's readings are retained but no longer current (FR-18), so the
   * whole section is de-emphasised.
   */
  stale?: boolean;
  children?: ReactNode;
}

/**
 * A controlled collapsible section, following the `CollapsibleSectionHeader`
 * idiom (title row, trailing caret, body hidden when collapsed).
 *
 * Deliberately *not* `CollapsibleSectionHeader` itself: that component owns
 * its expanded state and persists it under its own `vibe.ui.collapsible.*`
 * localStorage prefix, whereas the drawer's section state belongs to
 * `useMetricsDrawerStore` alongside the rest of the panel's view preferences
 * (FR-13). Keeping it controlled also keeps these views props-only.
 */
export function MetricsSection({
  panelId,
  title,
  summary,
  expanded,
  onToggle,
  stale = false,
  children,
}: MetricsSectionProps) {
  const bodyId = `metrics-section-${panelId}`;
  return (
    <section
      data-testid={`metrics-section-${panelId}`}
      data-stale={stale ? 'true' : undefined}
      className={cn(
        'flex flex-col border-t border-border',
        stale && 'opacity-60'
      )}
    >
      <button
        type="button"
        onClick={() => onToggle(panelId)}
        aria-expanded={expanded}
        aria-controls={bodyId}
        className="flex items-center justify-between w-full gap-half py-half text-left"
      >
        <span className="font-medium text-normal truncate">{title}</span>
        <span className="flex items-center gap-half min-w-0">
          {summary}
          <CaretDownIcon
            weight="fill"
            aria-hidden="true"
            className={cn(
              'size-icon-xs text-low transition-transform shrink-0',
              !expanded && '-rotate-90'
            )}
          />
        </span>
      </button>
      {expanded && (
        <div id={bodyId} className="flex flex-col gap-half pb-base">
          {children}
        </div>
      )}
    </section>
  );
}

/** A label / value row. Non-interactive, so it is a `div`, never a `button`. */
export function MetricsRow({
  label,
  value,
  className,
}: {
  label: string;
  value: string;
  className?: string;
}) {
  return (
    <div
      aria-label={`${label}: ${value}`}
      className={cn('flex items-center justify-between gap-half', className)}
    >
      <span aria-hidden="true" className="text-sm text-low truncate">
        {label}
      </span>
      <span
        aria-hidden="true"
        className="font-ibm-plex-mono text-sm text-normal tabular-nums shrink-0"
      >
        {value}
      </span>
    </div>
  );
}
