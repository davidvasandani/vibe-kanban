import { useCallback, useMemo, useRef } from 'react';
import { useQuery, keepPreviousData } from '@tanstack/react-query';
import { useJsonPatchWsStream } from '@/shared/hooks/useJsonPatchWsStream';
import { workspaceSummaryKeys } from '@/shared/hooks/workspaceSummaryKeys';
import { makeLocalApiRequest } from '@/shared/lib/localApiTransport';
import { useHostId } from '@/shared/providers/HostIdProvider';
import type {
  WorkspaceWithStatus,
  WorkspaceSummary,
  WorkspaceSummaryResponse,
  ApiResponse,
} from 'shared/types';

// UI-specific workspace type for sidebar display
export interface SidebarWorkspace {
  id: string;
  name: string;
  branch: string;
  createdAt: string;
  updatedAt: string;
  description: string;
  filesChanged?: number;
  linesAdded?: number;
  linesRemoved?: number;
  isRunning?: boolean;
  isPinned?: boolean;
  isArchived?: boolean;
  hasPendingApproval?: boolean;
  hasRunningDevServer?: boolean;
  hasUnseenActivity?: boolean;
  latestProcessCompletedAt?: string;
  latestProcessStatus?:
    | 'running'
    | 'completed'
    | 'failed'
    | 'killed'
    | 'interrupted'
    | 'indeterminate';
  prStatus?: 'open' | 'merged' | 'closed' | 'unknown';
  prNumber?: number;
  prUrl?: string;
}

// Keep the old export name for backwards compatibility
export type Workspace = SidebarWorkspace;

export interface UseWorkspacesResult {
  workspaces: SidebarWorkspace[];
  archivedWorkspaces: SidebarWorkspace[];
  isLoading: boolean;
  isConnected: boolean;
  error: string | null;
}

// State shape from the WebSocket stream
type WorkspacesState = {
  workspaces: Record<string, WorkspaceWithStatus>;
};

// Transform WorkspaceWithStatus to SidebarWorkspace, optionally merging summary data
function toSidebarWorkspace(
  ws: WorkspaceWithStatus,
  summary?: WorkspaceSummary
): SidebarWorkspace {
  return {
    id: ws.id,
    name: ws.name ?? ws.branch, // Use name if available, fallback to branch
    branch: ws.branch,
    createdAt: ws.created_at,
    updatedAt: ws.updated_at,
    description: '',
    // Use real stats from summary if available
    filesChanged: summary?.files_changed ?? undefined,
    linesAdded: summary?.lines_added ?? undefined,
    linesRemoved: summary?.lines_removed ?? undefined,
    // Real data from stream
    isRunning: ws.is_running,
    isPinned: ws.pinned,
    isArchived: ws.archived,
    // Additional data from summary
    hasPendingApproval: summary?.has_pending_approval,
    hasRunningDevServer: summary?.has_running_dev_server,
    hasUnseenActivity: summary?.has_unseen_turns,
    latestProcessCompletedAt: summary?.latest_process_completed_at ?? undefined,
    latestProcessStatus: summary?.latest_process_status ?? undefined,
    prStatus: summary?.pr_status ?? undefined,
    prNumber:
      summary?.pr_number != null ? Number(summary.pr_number) : undefined,
    prUrl: summary?.pr_url ?? undefined,
  };
}

/**
 * Per-id memo of the last `toSidebarWorkspace` call, so an unchanged workspace
 * keeps the same row object across renders.
 */
export type RowCache = Map<
  string,
  {
    ws: WorkspaceWithStatus;
    summary: WorkspaceSummary | undefined;
    row: SidebarWorkspace;
  }
>;

/**
 * Two `WorkspaceSummary` objects that carry the same values are interchangeable
 * for rendering. Identity comparison is not enough: the summaries query rebuilds
 * its `Map` and every object in it on each 15s poll (react-query does no
 * structural sharing for a `Map`), so an identity check would miss on every row
 * on every poll and defeat the row cache entirely.
 */
function sameSummary(
  a: WorkspaceSummary | undefined,
  b: WorkspaceSummary | undefined
): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  return (
    a.files_changed === b.files_changed &&
    a.lines_added === b.lines_added &&
    a.lines_removed === b.lines_removed &&
    a.has_pending_approval === b.has_pending_approval &&
    a.has_running_dev_server === b.has_running_dev_server &&
    a.has_unseen_turns === b.has_unseen_turns &&
    a.latest_session_id === b.latest_session_id &&
    a.latest_process_completed_at === b.latest_process_completed_at &&
    a.latest_process_status === b.latest_process_status &&
    a.pr_status === b.pr_status &&
    a.pr_number === b.pr_number &&
    a.pr_url === b.pr_url
  );
}

/**
 * Default ordering: pinned first, then newest `created_at`.
 *
 * This ordering is a contract, not a convenience — several consumers read it
 * positionally rather than re-sorting: `WorkspaceSelectionDialog` paginates the
 * list to 50 without sorting, `getNextWorkspaceId` picks an index-adjacent
 * workspace after archiving, `CreateModeProvider` takes the head element to seed
 * project selection, and the remote-web mobile list renders it as-is.
 *
 * The cost that mattered was never the sort, it was deriving the key inside the
 * comparator (`new Date(...)` per comparison, ~2·n·log n times). Keys are
 * precomputed once per row here instead.
 */
function sortSidebarRows(rows: SidebarWorkspace[]): SidebarWorkspace[] {
  return rows
    .map((row) => {
      const ts = Date.parse(row.createdAt);
      return {
        row,
        pinned: row.isPinned === true,
        ts: Number.isNaN(ts) ? -Infinity : ts,
      };
    })
    .sort((a, b) => {
      if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
      return b.ts - a.ts;
    })
    .map(({ row }) => row);
}

/** Exported for tests; not part of the hook's public surface. */
export function toSidebarWorkspaces(
  byId: Record<string, WorkspaceWithStatus> | undefined,
  summaries: Map<string, WorkspaceSummary>,
  cache: RowCache
): SidebarWorkspace[] {
  if (!byId) {
    cache.clear();
    return [];
  }

  const entries = Object.values(byId);
  const rows = entries.map((ws) => {
    const summary = summaries.get(ws.id);
    const cached = cache.get(ws.id);
    if (cached && cached.ws === ws && sameSummary(cached.summary, summary)) {
      return cached.row;
    }
    const row = toSidebarWorkspace(ws, summary);
    cache.set(ws.id, { ws, summary, row });
    return row;
  });

  // Prune ids that are no longer in the stream, so the cache cannot grow without
  // bound as workspaces are created and archived.
  if (cache.size > entries.length) {
    const live = new Set(entries.map((ws) => ws.id));
    for (const id of cache.keys()) {
      if (!live.has(id)) {
        cache.delete(id);
      }
    }
  }

  return sortSidebarRows(rows);
}

export const workspaceKeys = {
  all: ['workspaces'] as const,
};

// workspaceSummaryKeys is imported from @/shared/hooks/workspaceSummaryKeys

// Fetch workspace summaries from the API by archived status
async function fetchWorkspaceSummariesByArchived(
  archived: boolean,
  hostId: string | null
): Promise<Map<string, WorkspaceSummary>> {
  try {
    const basePath = hostId ? `/api/host/${hostId}` : '/api';
    const response = await makeLocalApiRequest(
      `${basePath}/workspaces/summaries`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ archived }),
      }
    );

    if (!response.ok) {
      console.warn('Failed to fetch workspace summaries:', response.status);
      return new Map();
    }

    const data: ApiResponse<WorkspaceSummaryResponse> = await response.json();
    if (!data.success || !data.data?.summaries) {
      return new Map();
    }

    const map = new Map<string, WorkspaceSummary>();
    for (const summary of data.data.summaries) {
      map.set(summary.workspace_id, summary);
    }
    return map;
  } catch (err) {
    console.warn('Error fetching workspace summaries:', err);
    return new Map();
  }
}

export function useWorkspaces(): UseWorkspacesResult {
  const hostId = useHostId();

  // Two separate WebSocket connections: one for active, one for archived
  // No limit param - we fetch all and slice on frontend so backfill works when archiving
  const apiBasePath = hostId ? `/api/host/${hostId}` : '/api';
  const activeEndpoint = `${apiBasePath}/workspaces/streams/ws?archived=false`;
  const archivedEndpoint = `${apiBasePath}/workspaces/streams/ws?archived=true`;

  const initialData = useCallback(
    (): WorkspacesState => ({ workspaces: {} }),
    []
  );

  const {
    data: activeData,
    isConnected: activeIsConnected,
    isInitialized: activeIsInitialized,
    error: activeError,
  } = useJsonPatchWsStream<WorkspacesState>(activeEndpoint, true, initialData);

  const {
    data: archivedData,
    isConnected: archivedIsConnected,
    isInitialized: archivedIsInitialized,
    error: archivedError,
  } = useJsonPatchWsStream<WorkspacesState>(
    archivedEndpoint,
    true,
    initialData
  );

  // Wait for both streams to be initialized before fetching summaries
  // Fetch summaries for active workspaces
  const { data: activeSummaries = new Map<string, WorkspaceSummary>() } =
    useQuery({
      queryKey: workspaceSummaryKeys.byArchived(false, hostId),
      queryFn: () => fetchWorkspaceSummariesByArchived(false, hostId),
      enabled: activeIsInitialized,
      staleTime: 1000,
      refetchInterval: 15000,
      refetchOnWindowFocus: false,
      refetchOnMount: 'always',
      placeholderData: keepPreviousData,
    });

  // Fetch summaries for archived workspaces
  const { data: archivedSummaries = new Map<string, WorkspaceSummary>() } =
    useQuery({
      queryKey: workspaceSummaryKeys.byArchived(true, hostId),
      queryFn: () => fetchWorkspaceSummariesByArchived(true, hostId),
      enabled: archivedIsInitialized,
      staleTime: 1000,
      refetchInterval: 15000,
      refetchOnWindowFocus: false,
      refetchOnMount: 'always',
      placeholderData: keepPreviousData,
    });

  // Row objects are reused when their inputs are reference-identical, so a
  // WebSocket patch or a summaries refetch only replaces the rows that actually
  // changed. Without this every row got a fresh object on every patch and every
  // 15s poll, which invalidated every downstream filter/sort memo and defeated
  // `React.memo` on the row component.
  const activeRowCache = useRef<RowCache>(new Map());
  const archivedRowCache = useRef<RowCache>(new Map());

  const workspaces = useMemo(
    () =>
      toSidebarWorkspaces(
        activeData?.workspaces,
        activeSummaries,
        activeRowCache.current
      ),
    [activeData, activeSummaries]
  );

  const archivedWorkspaces = useMemo(
    () =>
      toSidebarWorkspaces(
        archivedData?.workspaces,
        archivedSummaries,
        archivedRowCache.current
      ),
    [archivedData, archivedSummaries]
  );

  // isLoading is true when we haven't received initial data from either stream
  const isLoading = !activeIsInitialized || !archivedIsInitialized;

  // Combined connection status
  const isConnected = activeIsConnected && archivedIsConnected;

  // Combined error (show first error if any)
  const error = activeError || archivedError;

  return {
    workspaces,
    archivedWorkspaces,
    isLoading,
    isConnected,
    error,
  };
}
