import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  SpinnerIcon,
  PlusIcon,
  TrashIcon,
  PencilSimpleIcon,
} from '@phosphor-icons/react';
import { Input } from '@vibe/ui/components/Input';
import { Button } from '@vibe/ui/components/Button';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import type {
  GitHubOwnerToken,
  AppConfig,
  CreateGitHubOwnerTokenRequest,
  UpdateGitHubOwnerTokenRequest,
} from 'shared/types';
import { cn } from '@/shared/lib/utils';
import { SettingsCard, SettingsField } from './SettingsComponents';
import { useSettingsMachineClient } from './SettingsHostContext';
import { normalizeSecretReference } from './secretReference';

// Mirrors `utils::github_auth::is_valid_owner`: a GitHub login.
export const GITHUB_OWNER_PATTERN =
  /^[A-Za-z0-9](?:[A-Za-z0-9-]{0,37}[A-Za-z0-9])?$/;

// Same draft rules as organization Env Vars: drop quotes 1Password adds to a
// copied reference while typing, normalize fully on submit.
const unquoteDraft = (value: string) => {
  const reference = normalizeSecretReference(value);
  return reference !== null && reference !== value.trim() ? reference : value;
};

const submittedValue = (value: string) =>
  normalizeSecretReference(value) ?? value.trim();

const draftInputType = (value: string) =>
  normalizeSecretReference(value) === null ? 'password' : 'text';

function AppFields({
  app,
  onChange,
}: {
  app: AppConfig | null;
  onChange: (app: AppConfig | null) => void;
}) {
  return (
    <div className="space-y-2">
      <label className="text-sm">
        Credential type
        <select
          aria-label="Credential type"
          className="bg-secondary border border-border rounded-sm ml-2"
          value={app ? 'app' : 'pat'}
          onChange={(e) =>
            onChange(
              e.target.value === 'app'
                ? { app_id: '', installation_id: '', private_key_reference: '' }
                : null
            )
          }
        >
          <option value="pat">Personal access token</option>
          <option value="app">GitHub App installation</option>
        </select>
      </label>
      {app && (
        <div className="space-y-2">
          <Input
            aria-label="App ID"
            placeholder="App ID"
            value={app.app_id}
            onChange={(e) => onChange({ ...app, app_id: e.target.value })}
          />
          <Input
            aria-label="Installation ID"
            placeholder="Installation ID"
            value={app.installation_id}
            onChange={(e) =>
              onChange({ ...app, installation_id: e.target.value })
            }
          />
          <Input
            aria-label="Private key reference"
            placeholder="op://vault/item/private key"
            value={app.private_key_reference}
            onChange={(e) =>
              onChange({
                ...app,
                private_key_reference: unquoteDraft(e.target.value),
              })
            }
            autoComplete="off"
          />
        </div>
      )}
    </div>
  );
}
const appReady = (app: AppConfig) =>
  /^[1-9][0-9]*$/.test(app.app_id) &&
  /^[1-9][0-9]*$/.test(app.installation_id) &&
  normalizeSecretReference(app.private_key_reference) !== null;
const normalizedApp = (app: AppConfig) => ({
  ...app,
  private_key_reference: submittedValue(app.private_key_reference),
});

export function GitHubOwnerTokensCard() {
  const machineClient = useSettingsMachineClient();
  const queryClient = useQueryClient();
  const queryKey = [
    'github-owner-tokens',
    ...(machineClient?.queryScopeKey ?? ['machine', 'unselected']),
  ] as const;
  const [error, setError] = useState<string | null>(null);
  const [newOwner, setNewOwner] = useState('');
  const [newValue, setNewValue] = useState('');
  const [newApp, setNewApp] = useState<AppConfig | null>(null);
  const [editing, setEditing] = useState<{
    id: string;
    value: string;
    app: AppConfig | null;
  } | null>(null);

  const client = () => {
    if (!machineClient) throw new Error('Machine client is required');
    return machineClient;
  };

  const {
    data: tokens = [],
    isLoading,
    error: loadError,
  } = useQuery({
    queryKey,
    queryFn: () => client().listGitHubOwnerTokens(),
    enabled: machineClient != null,
  });

  const health = useQuery({
    queryKey: [...queryKey, 'health'],
    queryFn: () => client().getGitHubOwnerTokenHealth(),
    enabled: machineClient != null,
    staleTime: 60_000,
  });

  const onError = (err: unknown) =>
    setError(err instanceof Error ? err.message : 'Request failed');
  const onSettled = () => queryClient.invalidateQueries({ queryKey });

  const createToken = useMutation({
    mutationFn: (data: CreateGitHubOwnerTokenRequest) =>
      client().createGitHubOwnerToken(data),
    onSuccess: () => {
      setNewOwner('');
      setNewValue('');
      setNewApp(null);
    },
    onError,
    onSettled,
  });
  const updateToken = useMutation({
    mutationFn: (data: UpdateGitHubOwnerTokenRequest & { id: string }) =>
      client().updateGitHubOwnerToken(data.id, {
        value: data.value,
        app: data.app,
      }),
    onSuccess: () => setEditing(null),
    onError,
    onSettled,
  });
  const deleteToken = useMutation({
    mutationFn: (id: string) => client().deleteGitHubOwnerToken(id),
    onError,
    onSettled,
  });

  const handleAdd = () => {
    setError(null);
    const owner = newOwner.trim();
    if (!GITHUB_OWNER_PATTERN.test(owner)) {
      setError(
        'Owner must be a GitHub organization or user name: letters, digits or hyphens'
      );
      return;
    }
    if (tokens.some((t) => t.owner.toLowerCase() === owner.toLowerCase())) {
      setError(`A token for ${owner} already exists; edit it instead`);
      return;
    }
    createToken.mutate({
      owner,
      value: newApp ? '' : submittedValue(newValue),
      ...(newApp ? { app: normalizedApp(newApp) } : {}),
    });
  };

  const handleStartEdit = (token: GitHubOwnerToken) => {
    setError(null);
    setEditing({
      id: token.id,
      value: token.reference ?? '',
      app: token.app ?? null,
    });
  };

  const handleSaveEdit = () => {
    if (!editing) return;
    setError(null);
    updateToken.mutate({
      id: editing.id,
      value: editing.app ? '' : submittedValue(editing.value),
      ...(editing.app ? { app: normalizedApp(editing.app) } : {}),
    });
  };

  const handleDelete = (token: GitHubOwnerToken) => {
    const confirmed = window.confirm(
      `Delete the GitHub token for "${token.owner}"? New sessions will fall back to their existing GitHub authentication for this owner.`
    );
    if (!confirmed) return;
    setError(null);
    deleteToken.mutate(token.id);
  };

  return (
    <SettingsCard
      title="GitHub organization tokens"
      description="Choose a personal access token or GitHub App installation for each owner. Vibe Kanban and workspace gh/git commands use that owner's credential. App private keys stay on the coordinator and installation tokens refresh on demand. After changing credentials, start a new workspace session. Values are encrypted at rest."
    >
      {error && (
        <div className="bg-error/10 border border-error/50 rounded-sm p-3 text-error text-sm">
          {error}
        </div>
      )}

      {isLoading ? (
        <div className="flex items-center justify-center py-4 gap-2">
          <SpinnerIcon className="size-icon-sm animate-spin" />
          <span className="text-sm text-low">Loading…</span>
        </div>
      ) : loadError ? (
        // Never show a failed load as "no tokens": configured owners may be
        // blocking launches while the list cannot be read.
        <div className="bg-error/10 border border-error/50 rounded-sm p-3 text-error text-sm">
          Could not load GitHub organization tokens:{' '}
          {loadError instanceof Error ? loadError.message : 'Request failed'}
        </div>
      ) : tokens.length === 0 ? (
        <div className="text-sm text-low">No GitHub organization tokens.</div>
      ) : (
        <div className="space-y-2">
          {tokens.map((token) => {
            const isEditing = editing?.id === token.id;
            return (
              <div
                key={token.id}
                className="border border-border rounded-sm p-3 flex items-center gap-3"
              >
                <div className="flex-1 min-w-0">
                  <div className="font-mono text-sm text-high truncate">
                    {token.owner}
                  </div>
                  {isEditing ? (
                    <>
                      <AppFields
                        app={editing.app}
                        onChange={(app) => setEditing({ ...editing, app })}
                      />
                      {!editing.app && (
                        <Input
                          autoFocus
                          type={draftInputType(editing.value)}
                          placeholder="New token or op://vault/item/field"
                          value={editing.value}
                          onChange={(e) =>
                            setEditing({
                              ...editing,
                              id: token.id,
                              value: unquoteDraft(e.target.value),
                            })
                          }
                          className="mt-2"
                          autoComplete="off"
                        />
                      )}
                    </>
                  ) : token.app ? (
                    <div className="text-xs text-normal mt-1">
                      GitHub App {token.app.app_id}, installation{' '}
                      {token.app.installation_id}
                      <div className="font-mono break-all">
                        {token.app.private_key_reference}
                      </div>
                    </div>
                  ) : token.reference ? (
                    <div
                      className="text-xs text-normal font-mono mt-1 truncate"
                      title={token.reference}
                    >
                      {token.reference}
                    </div>
                  ) : (
                    <div className="text-xs text-low font-mono mt-1">
                      ••••••••
                    </div>
                  )}
                  {health.data?.owners
                    .filter(
                      (h) => h.owner.toLowerCase() === token.owner.toLowerCase()
                    )
                    .map((h) => (
                      <div key={h.owner} className="text-xs mt-2">
                        Credential health: {h.status}
                        {h.detail && <div>{h.detail}</div>}
                        {h.app && (
                          <div>
                            App reachable: {h.app.app_reachable ? 'yes' : 'no'};
                            installation found:{' '}
                            {h.app.installation_found ? 'yes' : 'no'}; token
                            minted: {h.app.token_minted ? 'yes' : 'no'};
                            permissions:{' '}
                            {h.app.missing_permissions.length
                              ? h.app.missing_permissions.join(', ')
                              : h.app.token_minted
                                ? 'present'
                                : 'not checked'}
                          </div>
                        )}
                      </div>
                    ))}
                </div>
                <div className="flex items-center gap-1 shrink-0">
                  {isEditing ? (
                    <>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => setEditing(null)}
                        disabled={updateToken.isPending}
                      >
                        Cancel
                      </Button>
                      <PrimaryButton
                        variant="secondary"
                        value="Save"
                        onClick={handleSaveEdit}
                        disabled={
                          updateToken.isPending ||
                          (editing.app
                            ? !appReady(editing.app)
                            : editing.value.trim().length === 0)
                        }
                      />
                    </>
                  ) : (
                    <>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => handleStartEdit(token)}
                        aria-label={`Edit ${token.owner}`}
                      >
                        <PencilSimpleIcon
                          className="size-icon-xs"
                          weight="bold"
                        />
                      </Button>
                      <button
                        type="button"
                        onClick={() => handleDelete(token)}
                        disabled={deleteToken.isPending}
                        aria-label={`Delete ${token.owner}`}
                        className={cn(
                          'flex items-center justify-center p-2 rounded-sm',
                          'text-error hover:bg-error/10',
                          'disabled:opacity-50 disabled:cursor-not-allowed'
                        )}
                      >
                        <TrashIcon className="size-icon-xs" weight="bold" />
                      </button>
                    </>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}

      <Button
        variant="ghost"
        onClick={() => void health.refetch()}
        disabled={health.isFetching}
      >
        Check credential health
      </Button>
      {health.error && (
        <div className="text-error text-sm">
          Could not check credential health.
        </div>
      )}
      <div className="border-t border-border pt-4 space-y-3">
        <AppFields app={newApp} onChange={setNewApp} />
        <SettingsField label="Add organization token">
          <div className="flex flex-col gap-2 md:flex-row">
            <Input
              placeholder="GitHub owner"
              value={newOwner}
              onChange={(e) => setNewOwner(e.target.value)}
              className="md:w-48 font-mono"
              autoComplete="off"
            />
            {!newApp && (
              <Input
                type={draftInputType(newValue)}
                placeholder="token or op://vault/item/field"
                value={newValue}
                onChange={(e) => setNewValue(unquoteDraft(e.target.value))}
                className="flex-1"
                autoComplete="off"
              />
            )}
            <PrimaryButton
              variant="secondary"
              value="Add"
              onClick={handleAdd}
              disabled={
                createToken.isPending ||
                machineClient == null ||
                newOwner.trim().length === 0 ||
                (newApp ? !appReady(newApp) : newValue.trim().length === 0)
              }
            >
              <PlusIcon className="size-icon-xs mr-1" weight="bold" />
            </PrimaryButton>
          </div>
        </SettingsField>
      </div>
    </SettingsCard>
  );
}
