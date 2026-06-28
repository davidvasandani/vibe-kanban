import { useEffect, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { configApi } from '@/shared/lib/api';

// How often to ask the backend which build it is currently serving. The
// frontend bundle is embedded in the same binary as /api/info, so whenever a
// deploy restarts the server both flip to the new git sha together — but an
// already-open tab keeps running the old JS until a full reload. Polling lets
// us notice the server moved on and prompt the user to refresh.
const DEPLOY_POLL_INTERVAL_MS = 30_000;

// Frozen at the first server version this browser session observes. Kept at
// module scope (not a ref) so a remount of the consuming component can't reset
// the baseline: the running JS bundle never changes without a full reload, so
// the boot version is a property of the page load, not of any component.
let bootVersion: string | null = null;

/**
 * Detects when the backend has been redeployed to a different git sha than the
 * one this page loaded with, so the UI can prompt for a refresh.
 *
 * @param hostId Optional host to query; defaults to the local/active backend.
 */
export function useDeployUpdateAvailable(hostId?: string | null): {
  updateAvailable: boolean;
} {
  const { data } = useQuery({
    queryKey: ['deploy-version', hostId ?? null],
    queryFn: () => configApi.getConfig(hostId),
    refetchInterval: DEPLOY_POLL_INTERVAL_MS,
    // Don't poll a backgrounded tab; it refetches on refocus, which is exactly
    // when the user might be about to interact and want a fresh build anyway.
    refetchIntervalInBackground: false,
  });

  const serverVersion = data?.version ?? null;
  const [updateAvailable, setUpdateAvailable] = useState(false);

  useEffect(() => {
    // 'dev' is the sentinel /api/info returns when no git sha was embedded
    // (local `cargo run` / unstamped builds). Never nag in that case.
    if (!serverVersion || serverVersion === 'dev') return;
    if (bootVersion === null) {
      bootVersion = serverVersion;
      return;
    }
    if (serverVersion !== bootVersion) {
      setUpdateAvailable(true);
    }
  }, [serverVersion]);

  return { updateAvailable };
}
