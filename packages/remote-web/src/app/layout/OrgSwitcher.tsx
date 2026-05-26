import {
  BuildingsIcon,
  CaretDownIcon,
  CheckIcon,
} from "@phosphor-icons/react";
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
} from "@vibe/ui/components/Dropdown";
import { cn } from "@/shared/lib/utils";

export interface OrgSwitcherOrganization {
  id: string;
  name: string;
}

interface OrgSwitcherProps {
  organizations: OrgSwitcherOrganization[];
  selectedOrgId: string | null;
  onSelect: (id: string) => void;
}

export function OrgSwitcher({
  organizations,
  selectedOrgId,
  onSelect,
}: OrgSwitcherProps) {
  if (organizations.length <= 1) {
    return null;
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          className="p-1 rounded-sm text-low hover:text-normal cursor-pointer focus:outline-none focus-visible:ring-2 focus-visible:ring-brand"
          aria-label="Switch organization"
        >
          <CaretDownIcon className="h-3.5 w-3.5" weight="bold" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        side="bottom"
        align="start"
        className="min-w-[200px]"
      >
        {organizations.map((org) => (
          <DropdownMenuItem
            key={org.id}
            icon={org.id === selectedOrgId ? CheckIcon : BuildingsIcon}
            onClick={() => onSelect(org.id)}
            className={cn(org.id === selectedOrgId && "bg-brand/10")}
          >
            {org.name}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
