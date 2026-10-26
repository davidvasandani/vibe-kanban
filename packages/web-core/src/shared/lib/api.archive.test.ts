import { beforeEach, describe, expect, it, vi } from 'vitest';
import { makeLocalApiRequest } from './localApiTransport';
import { workspacesApi } from './api';

vi.mock('./localApiTransport', () => ({ makeLocalApiRequest: vi.fn() }));

describe('workspace archive transport', () => {
  beforeEach(() => vi.mocked(makeLocalApiRequest).mockReset());

  it('uses automatic admission and returns a deferral without manual fallback', async () => {
    vi.mocked(makeLocalApiRequest).mockResolvedValue(
      Response.json({ success: true, data: 'blocked' })
    );
    expect(await workspacesApi.archiveAutomatically('local-1')).toBe('blocked');
    expect(makeLocalApiRequest).toHaveBeenCalledTimes(1);
    expect(makeLocalApiRequest).toHaveBeenCalledWith(
      '/api/workspaces/local-1/archive/automatic',
      expect.objectContaining({ method: 'POST' })
    );
  });

  it('surfaces automatic archive errors without retrying through manual update', async () => {
    vi.mocked(makeLocalApiRequest).mockResolvedValue(
      Response.json(
        { success: false, message: 'remote unavailable' },
        { status: 503 }
      )
    );
    await expect(
      workspacesApi.archiveAutomatically('local-1')
    ).rejects.toThrow();
    expect(makeLocalApiRequest).toHaveBeenCalledTimes(1);
  });

  it('preserves explicit manual archive transport', async () => {
    vi.mocked(makeLocalApiRequest).mockResolvedValue(
      Response.json({ success: true, data: { id: 'local-1', archived: true } })
    );
    await workspacesApi.update('local-1', { archived: true });
    expect(makeLocalApiRequest).toHaveBeenCalledTimes(1);
    expect(makeLocalApiRequest).toHaveBeenCalledWith(
      '/api/workspaces/local-1',
      expect.objectContaining({ method: 'PUT', body: '{"archived":true}' })
    );
  });
});
