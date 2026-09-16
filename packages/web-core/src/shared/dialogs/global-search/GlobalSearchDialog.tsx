import { useEffect, useState } from 'react';
import { useNavigate } from '@tanstack/react-router';
import {
  Command,
  CommandInput,
  CommandList,
  CommandItem,
  CommandGroup,
} from '@vibe/ui/components/Command';
import {
  Dialog,
  DialogContent,
  DialogTitle,
  DialogDescription,
} from '@vibe/ui/components/Dialog';
import {
  searchGlobally,
  searchResultHref,
  type GlobalSearchResponse,
  type GlobalSearchResult,
} from '@/shared/lib/globalSearch';
import { useOrganizationStore } from '@/shared/stores/useOrganizationStore';

export interface GlobalSearchProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  local: boolean;
  remote: boolean;
  onSelectOrganization?: (id: string) => void;
}

export function useGlobalSearchShortcut(onOpen: () => void) {
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (
        (event.ctrlKey || event.metaKey) &&
        event.shiftKey &&
        event.key.toLowerCase() === 'f'
      ) {
        event.preventDefault();
        event.stopPropagation();
        onOpen();
      }
    };
    window.addEventListener('keydown', handler, true);
    return () => window.removeEventListener('keydown', handler, true);
  }, [onOpen]);
}

export function GlobalSearchDialog({
  open,
  onOpenChange,
  local,
  remote,
  onSelectOrganization,
}: GlobalSearchProps) {
  const [query, setQuery] = useState('');
  const [data, setData] = useState<GlobalSearchResponse>();
  const [loading, setLoading] = useState(false);
  const navigate = useNavigate();
  const setOrg = useOrganizationStore((s) => s.setSelectedOrgId);
  useEffect(() => {
    const controller = new AbortController();
    setData(undefined);
    const active = open && [...query.trim()].length >= 2;
    setLoading(active);
    if (!active) return () => controller.abort();
    const timer = setTimeout(() => {
      void searchGlobally(query, local, remote, controller.signal)
        .then((result) => {
          if (!controller.signal.aborted) {
            setData(result);
            setLoading(false);
          }
        })
        .catch(() => {
          if (!controller.signal.aborted) {
            setData({ results: [], truncated: false, unavailable: ['Search'] });
            setLoading(false);
          }
        });
    }, 250);
    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [query, open, local, remote]);

  const select = (result: GlobalSearchResult) => {
    if (result.organization_id)
      (onSelectOrganization ?? setOrg)(result.organization_id);
    onOpenChange(false);
    void navigate({ href: searchResultHref(result) });
  };
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="overflow-hidden p-base">
        <DialogTitle>Global Search</DialogTitle>
        <DialogDescription>
          All accessible organizations, projects, workspaces and chat. Chat
          searches prompts and final replies.
        </DialogDescription>
        <Command shouldFilter={false}>
          <CommandInput
            aria-label="Global search query"
            placeholder="Search everywhere…"
            value={query}
            onValueChange={setQuery}
            maxLength={200}
          />
          <div role="status" className="px-base py-half text-low">
            {loading
              ? 'Searching…'
              : !data
                ? 'Enter at least 2 characters.'
                : data.results.length === 0
                  ? 'No results.'
                  : `${data.results.length} results`}
            {data?.truncated &&
              ' Showing the first 20 matches per category and source. Refine your search for more.'}
          </div>
          {data && data.unavailable.length > 0 && (
            <p role="alert" className="px-base py-half text-error">
              Some sources could not be searched: {data.unavailable.join(', ')}.
              Check host connections or try again.
            </p>
          )}
          <CommandList>
            {(['organization', 'project', 'workspace', 'chat'] as const).map(
              (kind) => {
                const results =
                  data?.results.filter((r) => r.kind === kind) ?? [];
                return (
                  results.length > 0 && (
                    <CommandGroup
                      key={kind}
                      heading={
                        {
                          organization: 'Organizations',
                          project: 'Projects',
                          workspace: 'Workspaces',
                          chat: 'Chat',
                        }[kind]
                      }
                    >
                      {results.map((result) => (
                        <CommandItem
                          key={`${result.hostId}:${kind}:${result.id}`}
                          value={`${result.hostId}:${kind}:${result.id}`}
                          onSelect={() => select(result)}
                          className="flex flex-col items-start gap-1"
                        >
                          <span className="text-high break-all">
                            {result.title}
                            {result.archived ? ' (archived)' : ''}
                          </span>
                          <span className="text-low break-all">
                            {result.hostName ? `${result.hostName} · ` : ''}
                            {result.context}
                            {result.kind === 'workspace' &&
                            result.hostId === undefined
                              ? ' · Open project context'
                              : ''}
                          </span>
                          {result.snippet && (
                            <span className="line-clamp-3 whitespace-pre-wrap break-all">
                              {result.snippet}
                            </span>
                          )}
                        </CommandItem>
                      ))}
                    </CommandGroup>
                  )
                );
              }
            )}
          </CommandList>
        </Command>
      </DialogContent>
    </Dialog>
  );
}
