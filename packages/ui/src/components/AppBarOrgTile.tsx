import { CaretDownIcon, CheckIcon, BuildingsIcon } from '@phosphor-icons/react';
import { cn } from '../lib/cn';
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
} from './Dropdown';
import { Tooltip } from './Tooltip';

export interface AppBarOrgTileOrganization {
  id: string;
  name: string;
}

interface AppBarOrgTileProps {
  organizations: AppBarOrgTileOrganization[];
  selectedOrgId: string | null;
  onSelect: (id: string) => void;
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

const tileBaseClassName =
  'group relative flex items-center justify-center w-10 h-10 rounded-lg text-sm font-semibold transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-brand bg-brand/15 text-brand';

/**
 * Organization tile rendered at the top of the AppBar rail.
 *
 * With multiple organizations it acts as a dropdown switcher; with a single
 * organization it renders a static tile. Returns null when there is no
 * organization to display.
 */
export function AppBarOrgTile({
  organizations,
  selectedOrgId,
  onSelect,
}: AppBarOrgTileProps) {
  const selectedOrg =
    organizations.find((org) => org.id === selectedOrgId) ?? organizations[0];

  if (!selectedOrg) {
    return null;
  }

  const initials = getOrgInitials(selectedOrg.name);

  if (organizations.length <= 1) {
    return (
      <Tooltip content={selectedOrg.name} side="right">
        <div className={tileBaseClassName} aria-label={selectedOrg.name}>
          {initials}
        </div>
      </Tooltip>
    );
  }

  return (
    <DropdownMenu>
      <Tooltip content={selectedOrg.name} side="right">
        <DropdownMenuTrigger asChild>
          <button
            type="button"
            className={cn(
              tileBaseClassName,
              'cursor-pointer hover:bg-brand/25'
            )}
            aria-label="Switch organization"
          >
            {initials}
            <CaretDownIcon
              className="absolute -bottom-0.5 -right-0.5 h-3 w-3 rounded-full bg-secondary p-px text-low opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100"
              weight="bold"
            />
          </button>
        </DropdownMenuTrigger>
      </Tooltip>
      <DropdownMenuContent side="right" align="start" className="min-w-[200px]">
        {organizations.map((org) => (
          <DropdownMenuItem
            key={org.id}
            icon={org.id === selectedOrg.id ? CheckIcon : BuildingsIcon}
            onClick={() => onSelect(org.id)}
            className={cn(org.id === selectedOrg.id && 'bg-brand/10')}
          >
            {org.name}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
