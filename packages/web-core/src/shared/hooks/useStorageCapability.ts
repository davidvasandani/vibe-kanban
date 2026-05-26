import { useQuery } from '@tanstack/react-query';
import { useUserSystem } from '@/shared/hooks/useUserSystem';
import { getRemoteApiUrl } from '@/shared/lib/remoteApi';

export type StorageBackend = 'azure' | 'filesystem';

export interface StorageCapability {
  attachmentsEnabled: boolean;
  backend: StorageBackend;
  isLoading: boolean;
}

interface RemoteHealthShape {
  status?: string;
  version?: string;
  attachments_enabled?: boolean;
}

async function fetchRemoteHealth(): Promise<RemoteHealthShape> {
  const base = getRemoteApiUrl();
  if (!base) return {};
  const res = await fetch(`${base}/v1/health`, {
    method: 'GET',
    cache: 'no-store',
  });
  if (!res.ok) return {};
  return (await res.json()) as RemoteHealthShape;
}

/**
 * Reports whether attachment storage is available for the requested backend.
 *
 * - `filesystem` reads from useUserSystem (the local deployment always
 *   filesystem-backs attachments).
 * - `azure` queries the public /v1/health endpoint on the remote/cloud
 *   server. When AZURE_STORAGE_ACCOUNT_NAME is unset on that server, the
 *   endpoint returns `attachments_enabled: false` and uploads should be
 *   disabled in the UI.
 */
export function useStorageCapability(
  backend: StorageBackend
): StorageCapability {
  const userSystem = useUserSystem();

  const { data, isLoading } = useQuery({
    queryKey: ['storage-capability', 'remote-health'],
    queryFn: fetchRemoteHealth,
    enabled: backend === 'azure',
    staleTime: 5 * 60 * 1000,
  });

  if (backend === 'filesystem') {
    return {
      attachmentsEnabled: userSystem.attachmentsEnabled,
      backend,
      isLoading: userSystem.loading,
    };
  }

  return {
    attachmentsEnabled: data?.attachments_enabled ?? false,
    backend,
    isLoading,
  };
}
