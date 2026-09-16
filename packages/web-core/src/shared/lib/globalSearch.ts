import { makeRequest, listRelayHosts } from '@/shared/lib/remoteApi';
import { makeLocalApiRequest } from '@/shared/lib/localApiTransport';

export interface GlobalSearchResult {
  kind: 'organization' | 'project' | 'workspace' | 'chat';
  id: string;
  title: string;
  context: string;
  snippet: string;
  organization_id?: string;
  project_id?: string | null;
  workspace_id?: string | null;
  session_id?: string | null;
  issue_id?: string | null;
  archived: boolean;
  hostId?: string | null;
  hostName?: string;
}
export interface SearchResponse {
  results: GlobalSearchResult[];
  truncated: boolean;
}
export interface GlobalSearchResponse extends SearchResponse {
  unavailable: string[];
}

// Some relay transports cannot cancel an in-flight WebRTC message. Race the
// operation itself so their transport timeout cannot hold healthy results back.
function withAbort<T>(operation: Promise<T>, signal: AbortSignal): Promise<T> {
  return new Promise((resolve, reject) => {
    const abort = () => reject(signal.reason ?? new Error('Search cancelled'));
    if (signal.aborted) abort();
    else signal.addEventListener('abort', abort, { once: true });
    operation.then(
      (value) => {
        signal.removeEventListener('abort', abort);
        resolve(value);
      },
      (error) => {
        signal.removeEventListener('abort', abort);
        reject(error);
      }
    );
  });
}

export async function searchGlobally(
  query: string,
  local: boolean,
  remote: boolean,
  signal: AbortSignal
): Promise<GlobalSearchResponse> {
  const q = query.trim();
  const output: GlobalSearchResponse = {
    results: [],
    truncated: false,
    unavailable: [],
  };
  if ([...q].length < 2) return output;
  const path = `global-search?${new URLSearchParams({ q })}`;
  const requestSignal = AbortSignal.any([signal, AbortSignal.timeout(12000)]);
  const collect = async (
    label: string,
    request: () => Promise<Response>,
    hostId?: string | null
  ) => {
    try {
      if (requestSignal.aborted) throw new Error('Search deadline reached');
      const response = await withAbort(request(), requestSignal);
      if (!response.ok) throw new Error('Search unavailable');
      const body = await withAbort(response.json(), requestSignal);
      const data: SearchResponse = hostId === undefined ? body : body.data;
      if (!data || !Array.isArray(data.results))
        throw new Error('Invalid search response');
      output.results.push(
        ...data.results.map((result) => ({
          ...result,
          hostId,
          hostName: hostId === undefined ? undefined : label,
        }))
      );
      output.truncated ||= data.truncated;
    } catch {
      if (!signal.aborted) output.unavailable.push(label);
    }
  };
  const initial: Promise<void>[] = [];
  const jobs: { label: string; run: () => Promise<void> }[] = [];
  if (local)
    initial.push(
      collect(
        'This host',
        () =>
          makeLocalApiRequest(`/api/${path}`, {
            hostScope: 'explicit',
            hostId: null,
            signal: requestSignal,
          }),
        null
      )
    );
  if (remote) {
    initial.push(
      collect('Organizations and projects', () =>
        makeRequest(`/v1/${path}`, { signal: requestSignal })
      )
    );
    try {
      const hosts = await withAbort(
        listRelayHosts({ signal: requestSignal }),
        requestSignal
      );
      for (const host of hosts) {
        if (host.status !== 'online') {
          output.unavailable.push(host.name);
          continue;
        }
        jobs.push({
          label: host.name,
          run: () =>
            collect(
              host.name,
              () =>
                makeLocalApiRequest(`/api/${path}`, {
                  hostScope: local ? 'explicit' : 'none',
                  hostId: host.id,
                  relayHostId: host.id,
                  signal: requestSignal,
                }),
              host.id
            ),
        });
      }
    } catch {
      output.unavailable.push('Host directory');
    }
  }
  // Limit concurrent host requests; a search never fans out unboundedly.
  let next = 0;
  await Promise.all([
    ...initial,
    ...Array.from({ length: Math.min(3, jobs.length) }, async () => {
      while (next < jobs.length && !requestSignal.aborted)
        await jobs[next++].run();
    }),
  ]);
  if (!signal.aborted)
    output.unavailable.push(...jobs.slice(next).map((job) => job.label));
  output.results = mergeSearchResults(output.results);
  output.results.sort(
    (a, b) =>
      a.kind.localeCompare(b.kind) ||
      a.title.localeCompare(b.title) ||
      a.id.localeCompare(b.id)
  );
  return output;
}

// Prefer a directly navigable host result to the same cloud metadata row.
// UUIDs are workspace identity across relay routes to a shared database.
export function mergeSearchResults(
  results: GlobalSearchResult[]
): GlobalSearchResult[] {
  const metadata = new Map(
    results
      .filter(
        (r) =>
          r.kind === 'workspace' && r.hostId === undefined && r.workspace_id
      )
      .map((r) => [r.workspace_id, r])
  );
  const directIds = new Set(
    results
      .filter((r) => r.kind === 'workspace' && r.hostId !== undefined)
      .map((r) => r.workspace_id)
  );
  const seen = new Set<string>();
  return [...results]
    .sort((a, b) => (a.hostId ?? '').localeCompare(b.hostId ?? ''))
    .flatMap((result) => {
      if (
        result.kind === 'workspace' &&
        result.hostId === undefined &&
        directIds.has(result.workspace_id)
      )
        return [];
      const key = `${result.kind}:${result.id}`;
      if (seen.has(key)) return [];
      seen.add(key);
      const parent =
        result.workspace_id && result.hostId !== undefined
          ? metadata.get(result.workspace_id)
          : undefined;
      return [
        {
          ...result,
          ...(parent
            ? {
                organization_id: parent.organization_id,
                project_id: parent.project_id,
                context: `${parent.context} / ${result.context}`,
              }
            : {}),
        },
      ];
    });
}

export function searchResultHref(result: GlobalSearchResult): string {
  if (result.hostId !== undefined && result.workspace_id) {
    const prefix = result.hostId
      ? `/hosts/${encodeURIComponent(result.hostId)}`
      : '';
    const session = result.session_id
      ? `?searchSessionId=${encodeURIComponent(result.session_id)}`
      : '';
    return `${prefix}/workspaces/${encodeURIComponent(result.workspace_id)}${session}`;
  }
  if (result.project_id) {
    return result.issue_id
      ? `/projects/${encodeURIComponent(result.project_id)}/issues/${encodeURIComponent(result.issue_id)}`
      : `/projects/${encodeURIComponent(result.project_id)}`;
  }
  return '/';
}
