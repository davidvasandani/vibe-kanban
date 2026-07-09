import { useState } from 'react';
import { CaretDownIcon, CaretUpIcon } from '@phosphor-icons/react';
import { cn } from '../lib/cn';
import { Tooltip } from './Tooltip';

export interface AppBarOrgTileOrganization {
  id: string;
  name: string;
}

interface AppBarOrgTileProps {
  organizations: AppBarOrgTileOrganization[];
  selectedOrgId: string | null;
  onSelect: (id: string) => void;
  /**
   * Whether the org section is expanded to show every organization as a tile.
   * Only used in the controlled mode (see `onToggleExpanded`).
   */
  expanded?: boolean;
  /**
   * Toggles the expanded/collapsed state. When provided, the component is
   * controlled and this owns the state; when omitted, the component manages
   * its own expand state internally so switching still works.
   */
  onToggleExpanded?: () => void;
}

function getOrgInitials(name: string): string {
  const trimmed = name.trim();
  if (!trimmed) return '??';
  const words = trimmed.split(/\s+/);
  if (words.length >= 2) {
    return (words[0].charAt(0) + words[1].charAt(0)).toUpperCase();
  }
  return trimmed.slice(0, 2).toUpperCase();
}

/**
 * Derive a stable HSL-triple string (e.g. `"210 65% 55%"`) from an org id.
 * Organizations have no stored color (unlike projects), so we hash the id to a
 * hue while keeping saturation/lightness fixed and tuned for the dark rail.
 * Returned in the same format that project tiles feed into `hsl(...)`.
 */
function getOrgColor(id: string): string {
  let hash = 0;
  for (let i = 0; i < id.length; i++) {
    hash = (hash * 31 + id.charCodeAt(i)) | 0;
  }
  const hue = Math.abs(hash) % 360;
  return `${hue} 65% 55%`;
}

// Matches the project-tile recipe in AppBar.tsx so org tiles read identically.
const orgTileBaseClassName =
  'flex items-center justify-center w-10 h-10 rounded-lg text-sm font-medium transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-brand';

function OrgTileButton({
  org,
  isActive,
  onClick,
}: {
  org: AppBarOrgTileOrganization;
  isActive: boolean;
  onClick: () => void;
}) {
  const color = getOrgColor(org.id);
  return (
    <Tooltip content={org.name} side="right">
      <button
        type="button"
        onClick={onClick}
        className={cn(
          orgTileBaseClassName,
          'cursor-pointer',
          isActive ? '' : 'bg-primary text-normal hover:opacity-80'
        )}
        style={
          isActive
            ? {
                color: `hsl(${color})`,
                backgroundColor: `hsl(${color} / 0.2)`,
              }
            : undefined
        }
        aria-label={org.name}
        aria-current={isActive ? 'true' : undefined}
      >
        {getOrgInitials(org.name)}
      </button>
    </Tooltip>
  );
}

function OrgSectionLabel() {
  return (
    <p className="w-10 text-center text-[9px] font-medium leading-none tracking-wide text-low">
      Orgs
    </p>
  );
}

function ExpandToggle({
  expanded,
  onClick,
}: {
  expanded: boolean;
  onClick: () => void;
}) {
  const Icon = expanded ? CaretUpIcon : CaretDownIcon;
  return (
    <Tooltip
      content={expanded ? 'Hide organizations' : 'Show organizations'}
      side="right"
    >
      <button
        type="button"
        onClick={onClick}
        className={cn(
          'flex items-center justify-center w-10 h-5 rounded-md',
          'text-low hover:text-normal hover:bg-tertiary transition-colors cursor-pointer',
          'focus:outline-none focus-visible:ring-2 focus-visible:ring-brand'
        )}
        aria-label={expanded ? 'Hide organizations' : 'Show organizations'}
        aria-expanded={expanded}
      >
        <Icon className="h-3.5 w-3.5" weight="bold" />
      </button>
    </Tooltip>
  );
}

/**
 * Organization section at the top of the AppBar rail.
 *
 * - No organizations: renders nothing.
 * - One organization: a single static tile.
 * - Multiple organizations: collapsed shows the active org tile plus an expand
 *   toggle; expanded shows every organization as a project-styled tile for
 *   one-click switching (no dropdown), with a collapse toggle.
 */
export function AppBarOrgTile({
  organizations,
  selectedOrgId,
  onSelect,
  expanded = false,
  onToggleExpanded,
}: AppBarOrgTileProps) {
  // Uncontrolled fallback: when the caller does not own the expand state, the
  // component manages it so multi-org switching still works.
  const [internalExpanded, setInternalExpanded] = useState(false);

  const selectedOrg =
    organizations.find((org) => org.id === selectedOrgId) ?? organizations[0];

  if (!selectedOrg) {
    return null;
  }

  // Single org: nothing to switch between — a static tile.
  if (organizations.length <= 1) {
    return <OrgTileButton org={selectedOrg} isActive onClick={() => {}} />;
  }

  const isControlled = onToggleExpanded !== undefined;
  const isExpanded = isControlled ? expanded : internalExpanded;
  const toggle = () => {
    if (isControlled) {
      onToggleExpanded();
    } else {
      setInternalExpanded((prev) => !prev);
    }
  };

  if (isExpanded) {
    return (
      <div className="flex flex-col items-center gap-base">
        <OrgSectionLabel />
        {organizations.map((org) => (
          <OrgTileButton
            key={org.id}
            org={org}
            isActive={org.id === selectedOrg.id}
            onClick={() => onSelect(org.id)}
          />
        ))}
        <ExpandToggle expanded onClick={toggle} />
      </div>
    );
  }

  return (
    <div className="flex flex-col items-center gap-1">
      <OrgTileButton org={selectedOrg} isActive onClick={toggle} />
      <ExpandToggle expanded={false} onClick={toggle} />
    </div>
  );
}
