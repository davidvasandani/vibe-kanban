import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  searchGlobally,
  searchResultHref,
  mergeSearchResults,
  type GlobalSearchResult,
} from './globalSearch';
import { makeRequest, listRelayHosts } from './remoteApi';
import { makeLocalApiRequest } from './localApiTransport';
vi.mock('./remoteApi', () => ({
  makeRequest: vi.fn(),
  listRelayHosts: vi.fn(),
}));
vi.mock('./localApiTransport', () => ({ makeLocalApiRequest: vi.fn() }));
const result: GlobalSearchResult = {
  kind: 'chat',
  id: 'turn',
  title: 'Workspace',
  context: 'User',
  snippet: 'needle',
  workspace_id: 'w',
  session_id: 's',
  archived: true,
};
const response = (body: unknown) => new Response(JSON.stringify(body));
beforeEach(() => vi.resetAllMocks());
describe('global search', () => {
  it('does not request data for whitespace or short queries', async () => {
    expect(
      (await searchGlobally(' x ', true, true, new AbortController().signal))
        .results
    ).toEqual([]);
    expect(makeRequest).not.toHaveBeenCalled();
    expect(makeLocalApiRequest).not.toHaveBeenCalled();
    expect(listRelayHosts).not.toHaveBeenCalled();
  });
  it('preserves local scope and literal query encoding independently of current host', async () => {
    vi.mocked(makeLocalApiRequest).mockResolvedValue(
      response({ data: { results: [result], truncated: true } })
    );
    const found = await searchGlobally(
      ' %_ ',
      true,
      false,
      new AbortController().signal
    );
    expect(makeLocalApiRequest).toHaveBeenCalledWith(
      '/api/global-search?q=%25_',
      expect.objectContaining({ hostScope: 'explicit', hostId: null })
    );
    expect(found.results[0].hostId).toBeNull();
    expect(found.truncated).toBe(true);
  });
  it('searches every online host and reports offline and failed sources', async () => {
    vi.mocked(listRelayHosts).mockResolvedValue([
      { id: 'h1', name: 'First', status: 'online' },
      { id: 'h2', name: 'Second', status: 'online' },
      { id: 'h3', name: 'Offline', status: 'offline' },
    ] as Awaited<ReturnType<typeof listRelayHosts>>);
    vi.mocked(makeRequest).mockResolvedValue(new Response('', { status: 500 }));
    vi.mocked(makeLocalApiRequest).mockImplementation(async (_, options) => {
      if (options?.hostId === 'h2') throw new Error('Unpaired');
      return response({ data: { results: [result], truncated: false } });
    });
    const found = await searchGlobally(
      'needle',
      false,
      true,
      new AbortController().signal
    );
    expect(found.results[0].hostId).toBe('h1');
    expect(found.unavailable.sort()).toEqual([
      'Offline',
      'Organizations and projects',
      'Second',
    ]);
    expect(makeLocalApiRequest).toHaveBeenCalledTimes(2);
  });
  it('routes chat to its host and session and remote metadata to the right project', () => {
    expect(searchResultHref({ ...result, hostId: 'h' })).toBe(
      '/hosts/h/workspaces/w?searchSessionId=s'
    );
    expect(searchResultHref({ ...result, hostId: null })).toBe(
      '/workspaces/w?searchSessionId=s'
    );
    expect(
      searchResultHref({ ...result, kind: 'project', project_id: 'p' })
    ).toBe('/projects/p');
    expect(
      searchResultHref({
        ...result,
        kind: 'issue',
        project_id: 'p',
        issue_id: 'i',
      })
    ).toBe('/projects/p/issues/i');
  });
});

it('starts healthy local search without waiting for the remote host directory', async () => {
  let failDirectory!: (error: Error) => void;
  vi.mocked(listRelayHosts).mockImplementation(
    () =>
      new Promise((_, reject) => {
        failDirectory = reject;
      })
  );
  vi.mocked(makeRequest).mockResolvedValue(new Response('', { status: 503 }));
  vi.mocked(makeLocalApiRequest).mockResolvedValue(
    response({ data: { results: [result], truncated: false } })
  );
  const pending = searchGlobally(
    'needle',
    true,
    true,
    new AbortController().signal
  );
  expect(makeLocalApiRequest).toHaveBeenCalledOnce();
  failDirectory(new Error('Timeout'));
  const found = await pending;
  expect(found.results).toHaveLength(1);
  expect(found.unavailable).toContain('Host directory');
});

it('combines cloud context with direct workspace results and collapses duplicate host routes', () => {
  const workspace = {
    ...result,
    id: 'workspace-row',
    kind: 'workspace' as const,
    hostId: 'host',
  };
  const merged = mergeSearchResults([
    {
      ...workspace,
      hostId: undefined,
      id: 'remote-row',
      organization_id: 'org',
      project_id: 'project',
      context: 'Org / Project',
    },
    workspace,
    { ...workspace, hostId: null },
  ]);
  expect(merged).toHaveLength(1);
  expect(merged[0]).toMatchObject({
    hostId: null,
    organization_id: 'org',
    project_id: 'project',
  });
});

it('enforces its deadline even when a relay ignores AbortSignal', async () => {
  vi.useFakeTimers();
  const controller = new AbortController();
  const originalTimeout = AbortSignal.timeout;
  vi.spyOn(AbortSignal, 'timeout').mockImplementation(() => {
    setTimeout(() => controller.abort(new Error('Deadline')), 12000);
    return controller.signal;
  });
  try {
    vi.mocked(listRelayHosts).mockResolvedValue(
      Array.from({ length: 5 }, (_, i) => ({
        id: `h${i}`,
        name: `Host ${i}`,
        status: 'online',
      })) as Awaited<ReturnType<typeof listRelayHosts>>
    );
    vi.mocked(makeRequest).mockResolvedValue(
      response({ results: [], truncated: false })
    );
    vi.mocked(makeLocalApiRequest).mockImplementation(async (_, options) => {
      if (options?.hostId === null)
        return response({ data: { results: [result], truncated: false } });
      return new Promise(() => {});
    });
    const pending = searchGlobally(
      'needle',
      true,
      true,
      new AbortController().signal
    );
    await vi.advanceTimersByTimeAsync(12001);
    const found = await pending;
    expect(found.results).toHaveLength(1);
    expect(found.unavailable).toHaveLength(5);
    expect(makeLocalApiRequest).toHaveBeenCalledTimes(4);
  } finally {
    vi.mocked(AbortSignal.timeout).mockRestore();
    AbortSignal.timeout = originalTimeout;
    vi.useRealTimers();
  }
});
