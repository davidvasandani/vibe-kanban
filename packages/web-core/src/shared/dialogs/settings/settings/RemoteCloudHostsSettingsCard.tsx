import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { useNavigate, useParams } from '@tanstack/react-router';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import {
  usePairRemoteCloudHostMutation,
  useRemoveRemoteCloudHostMutation,
} from '@/shared/hooks/useRemoteCloudHosts';
import { relayApi } from '@/shared/lib/api';
import {
  SettingsField,
  SettingsInput,
  SettingsSelect,
} from './SettingsComponents';
import { PairingCodeInput } from './PairingCodeInput';
import { normalizeEnrollmentCode } from '@/shared/lib/relayPake';
import { useUserSystem } from '@/shared/hooks/useUserSystem';
import {
  usePairRelayHostMutation,
  useRelayRemoteHostsQuery,
  useRelayRemotePairedHostsQuery,
  useRemovePairedRelayHostMutation,
} from './useRelayRemoteHostMutations';
import { createRelayClientIdentity } from '@/shared/lib/relayClientIdentity';

export function RemoteCloudHostsSettingsCardContent({
  initialHostId,
  mode = 'local',
  onClose,
  showPairing = true,
}: {
  showPairing?: boolean;
  initialHostId?: string;
  mode?: 'local' | 'remote';
  onClose?: () => void;
}) {
  const { t } = useTranslation(['settings', 'common']);
  const navigate = useNavigate();
  const { hostId: routeHostId } = useParams({ strict: false });
  const [hostName, setHostName] = useState('');
  const [selectedHostId, setSelectedHostId] = useState<string | undefined>();
  const [pairingCode, setPairingCode] = useState('');
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const [removingHostId, setRemovingHostId] = useState<string | null>(null);
  const hasAppliedInitialHostRef = useRef(false);
  const { machineId } = useUserSystem();

  const isRemoteMode = mode === 'remote';
  const relayQuery = useQuery({
    ...useRelayRemoteHostsQuery(),
    refetchInterval: 10000,
  });
  const relayHosts = useMemo(() => relayQuery.data ?? [], [relayQuery.data]);
  const relayHostsLoading = relayQuery.isLoading;
  const localQuery = useQuery({
    queryKey: ['relay', 'local', 'paired-hosts'],
    queryFn: () => relayApi.listPairedRelayHosts(),
    enabled: !isRemoteMode,
  });
  const remoteQuery = useQuery({
    ...useRelayRemotePairedHostsQuery(),
    enabled: isRemoteMode,
  });
  const pairedQuery = isRemoteMode ? remoteQuery : localQuery;
  const { mutateAsync: pairLocalHost, isPending: isPairingLocal } =
    usePairRemoteCloudHostMutation();
  const { mutateAsync: removeLocalHost, isPending: isRemovingLocal } =
    useRemoveRemoteCloudHostMutation();
  const { mutateAsync: pairRemoteHost, isPending: isPairingRemote } =
    usePairRelayHostMutation();
  const { mutateAsync: removeRemoteHost, isPending: isRemovingRemote } =
    useRemovePairedRelayHostMutation();
  const isDevMode = import.meta.env.DEV;
  const pairableRelayHosts = useMemo(() => {
    if (isRemoteMode || !machineId || isDevMode) {
      return relayHosts;
    }

    return relayHosts.filter((host) => host.machine_id !== machineId);
  }, [isDevMode, isRemoteMode, machineId, relayHosts]);
  const defaultClientName = useMemo(
    () => createRelayClientIdentity().clientName,
    []
  );

  useEffect(() => {
    if (pairableRelayHosts.length === 0) {
      setSelectedHostId(undefined);
      return;
    }

    if (!selectedHostId) {
      setSelectedHostId(pairableRelayHosts[0].id);
      return;
    }

    if (!pairableRelayHosts.some((host) => host.id === selectedHostId)) {
      setSelectedHostId(pairableRelayHosts[0].id);
    }
  }, [pairableRelayHosts, selectedHostId]);

  useEffect(() => {
    if (!initialHostId || hasAppliedInitialHostRef.current) {
      return;
    }

    if (relayHostsLoading || relayQuery.isError) {
      return;
    }

    const initialHost = pairableRelayHosts.find(
      (host) => host.id === initialHostId
    );
    if (!initialHost) {
      hasAppliedInitialHostRef.current = true;
      return;
    }

    setSelectedHostId(initialHost.id);
    setErrorMessage(null);
    setSuccessMessage(null);
    hasAppliedInitialHostRef.current = true;
  }, [
    initialHostId,
    pairableRelayHosts,
    relayHostsLoading,
    relayQuery.isError,
  ]);

  const relayHostOptions = useMemo(
    () =>
      pairableRelayHosts.map((host) => ({
        value: host.id,
        label: host.name,
      })),
    [pairableRelayHosts]
  );

  const connectedHosts = useMemo(() => {
    const liveById = new Map(relayHosts.map((host) => [host.id, host]));
    return (pairedQuery.data ?? [])
      .map((host) => {
        const liveHost = liveById.get(host.host_id);
        return {
          id: host.host_id,
          name: liveHost?.name ?? host.host_name ?? host.host_id,
          status:
            relayQuery.isError || !relayQuery.data
              ? 'unknown'
              : (liveHost?.status ?? 'offline'),
          pairedAt: host.paired_at,
        };
      })
      .sort((a, b) => a.name.localeCompare(b.name) || a.id.localeCompare(b.id));
  }, [pairedQuery.data, relayHosts, relayQuery.isError, relayQuery.data]);

  const isLoading = pairedQuery.isLoading;
  const refresh = () =>
    Promise.all([pairedQuery.refetch(), relayQuery.refetch()]);
  const isPairing = isRemoteMode ? isPairingRemote : isPairingLocal;
  const isRemoving = isRemoteMode ? isRemovingRemote : isRemovingLocal;

  const canSubmitPairing =
    !!selectedHostId &&
    normalizeEnrollmentCode(pairingCode).length === 6 &&
    !isPairing &&
    !relayQuery.isError &&
    !relayHostsLoading;

  const resetForm = () => {
    setHostName('');
    setPairingCode('');
  };

  const handleConnect = async () => {
    setErrorMessage(null);
    setSuccessMessage(null);

    if (!selectedHostId) {
      setErrorMessage(
        t(
          'settings.relay.remoteCloudHost.hostRequired',
          'Select a host to connect.'
        )
      );
      return;
    }

    const selectedHost = pairableRelayHosts.find(
      (host) => host.id === selectedHostId
    );
    if (!selectedHost) {
      setErrorMessage(
        t(
          'settings.relay.remoteCloudHost.hostMissing',
          'Selected host is no longer available.'
        )
      );
      return;
    }

    const normalizedCode = normalizeEnrollmentCode(pairingCode);
    const effectiveHostName = hostName.trim() || defaultClientName;

    try {
      if (isRemoteMode) {
        await pairRemoteHost({
          hostId: selectedHost.id,
          hostName: effectiveHostName,
          normalizedCode,
        });
      } else {
        await pairLocalHost({
          host_id: selectedHost.id,
          host_name: effectiveHostName,
          enrollment_code: normalizedCode,
        });
      }
      await refresh();
      setSuccessMessage(
        t(
          'settings.relay.remoteCloudHost.connectSuccess',
          'Remote Cloud Host connected.'
        )
      );
      resetForm();
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  };

  const handleRemove = async (hostId: string) => {
    const confirmed = window.confirm(
      t('settings.relay.management.removeConfirm', {
        name: connectedHosts.find((host) => host.id === hostId)?.name ?? hostId,
      })
    );

    if (!confirmed) {
      return;
    }

    setRemovingHostId(hostId);
    setErrorMessage(null);
    setSuccessMessage(null);

    try {
      if (isRemoteMode) {
        await removeRemoteHost(hostId);
      } else {
        await removeLocalHost(hostId);
      }
      await refresh();
      if (hostId === routeHostId) {
        onClose?.();
        void navigate({ to: '/' });
      }
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setRemovingHostId(null);
    }
  };

  const handleGoToHostWorkspaces = (hostId: string, status?: string) => {
    if (status !== 'online') {
      return;
    }

    onClose?.();
    void navigate({
      to: '/hosts/$hostId/workspaces',
      params: { hostId },
    });
  };

  return (
    <div className="space-y-4">
      {successMessage && (
        <div className="bg-success/10 border border-success/50 rounded-sm p-3 text-success text-sm">
          {successMessage}
        </div>
      )}

      {errorMessage && (
        <div
          role="alert"
          className="bg-error/10 border border-error/50 rounded-sm p-3 text-error text-sm"
        >
          {errorMessage}
        </div>
      )}

      <section
        className="space-y-3"
        aria-label={t('settings.relay.management.title')}
      >
        <div className="flex items-center justify-between gap-3">
          <div>
            <h3 className="text-base font-semibold text-high">
              {t('settings.relay.management.title')}
            </h3>
            <p className="text-sm text-low">
              {t('settings.relay.management.description')}
            </p>
          </div>
          <PrimaryButton
            variant="secondary"
            value={t('settings.relay.management.refresh')}
            onClick={() => void refresh()}
            disabled={pairedQuery.isFetching || relayQuery.isFetching}
            actionIcon={
              pairedQuery.isFetching || relayQuery.isFetching
                ? 'spinner'
                : undefined
            }
          />
        </div>
        {isLoading && (
          <p role="status" className="text-sm text-low">
            {t('settings.relay.management.loading')}
          </p>
        )}
        {pairedQuery.isError && (
          <p role="alert" className="text-sm text-error">
            {t('settings.relay.management.loadError')}
          </p>
        )}
        {relayQuery.isError && (
          <p role="alert" className="text-sm text-error">
            {t('settings.relay.management.statusError')}
          </p>
        )}
        {!isLoading && !pairedQuery.isError && connectedHosts.length === 0 && (
          <p className="rounded-sm border border-border p-3 text-sm text-low">
            {t('settings.relay.management.empty')}
          </p>
        )}
        <ul className="space-y-2">
          {connectedHosts.map((host) => {
            const date = host.pairedAt ? new Date(host.pairedAt) : null;
            return (
              <li
                key={host.id}
                className="rounded-sm border border-border bg-secondary/30 p-3 flex flex-wrap items-center justify-between gap-3"
              >
                <div className="min-w-0 flex-1">
                  <p className="text-base font-medium text-high break-words">
                    {host.name}
                  </p>
                  <p className="text-sm text-low break-all">{host.id}</p>
                  <p
                    className={
                      host.status === 'online'
                        ? 'text-sm text-success'
                        : 'text-sm text-low'
                    }
                  >
                    {host.status === 'online'
                      ? t('settings.relay.management.online')
                      : host.status === 'offline'
                        ? t('settings.relay.management.offline')
                        : host.status === 'unpaired'
                          ? t('settings.relay.management.unpaired')
                          : t('settings.relay.management.unknown')}
                    {date && Number.isFinite(date.getTime()) && (
                      <>
                        {' '}
                        ·{' '}
                        {t('settings.relay.management.pairedAt', {
                          date: date.toLocaleDateString(),
                        })}
                      </>
                    )}
                  </p>
                </div>
                <div className="flex flex-wrap gap-2">
                  <PrimaryButton
                    variant="secondary"
                    value={t('settings.relay.management.open')}
                    onClick={() =>
                      handleGoToHostWorkspaces(host.id, host.status)
                    }
                    disabled={host.status !== 'online' || isRemoving}
                  />
                  <PrimaryButton
                    variant="tertiary"
                    value={t('settings.relay.remoteCloudHost.remove', 'Remove')}
                    onClick={() => void handleRemove(host.id)}
                    disabled={isRemoving}
                    actionIcon={
                      removingHostId === host.id ? 'spinner' : undefined
                    }
                  />
                </div>
              </li>
            );
          })}
        </ul>
      </section>

      {showPairing && (
        <div className="space-y-4 border-t border-border pt-4">
          <SettingsField
            label={t('settings.relay.client.pair.hostLabel', 'Host to pair to')}
          >
            <SettingsSelect
              value={selectedHostId}
              options={relayHostOptions}
              onChange={setSelectedHostId}
              placeholder={t(
                'settings.relay.remoteCloudHost.hostPlaceholder',
                relayHostsLoading
                  ? 'Loading hosts...'
                  : pairableRelayHosts.length === 0
                    ? 'No hosts available'
                    : 'Select a host'
              )}
              disabled={
                isPairing ||
                relayHostsLoading ||
                relayQuery.isError ||
                relayHostOptions.length === 0
              }
            />
          </SettingsField>

          {!relayHostsLoading &&
            !relayQuery.isError &&
            pairableRelayHosts.length === 0 && (
              <p className="text-sm text-low">
                {t(
                  'settings.relay.remoteCloudHost.hostsUnavailable',
                  'No hosts found yet. Make sure another device is running as a host and has paired with this account.'
                )}
              </p>
            )}

          {selectedHostId && (
            <>
              <SettingsField
                label={t(
                  'settings.relay.client.pair.nameLabel',
                  'How this device appears on that host (optional)'
                )}
              >
                <SettingsInput
                  value={hostName}
                  onChange={setHostName}
                  placeholder={t(
                    'settings.relay.remoteCloudHost.namePlaceholder',
                    defaultClientName
                  )}
                />
              </SettingsField>

              <SettingsField
                label={t(
                  'settings.relay.client.pair.pairingCodeLabel',
                  'Pairing code from the host'
                )}
                description={t(
                  'settings.relay.client.pair.pairingCodeHelp',
                  'Enter the 6-character code shown on the host you want to connect to.'
                )}
              >
                <PairingCodeInput
                  value={pairingCode}
                  onChange={setPairingCode}
                />
              </SettingsField>

              <div className="flex items-center gap-2">
                <PrimaryButton
                  value={t(
                    'settings.relay.client.pair.confirm',
                    'Pair this device'
                  )}
                  onClick={() => void handleConnect()}
                  disabled={!canSubmitPairing}
                  actionIcon={isPairing ? 'spinner' : undefined}
                />
                <PrimaryButton
                  variant="tertiary"
                  value={t('common:buttons.cancel')}
                  onClick={resetForm}
                  disabled={isPairing}
                />
              </div>
            </>
          )}
        </div>
      )}
    </div>
  );
}
