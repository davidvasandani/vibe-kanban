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
import type { GitHubOwnerToken } from 'shared/types';
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
  const [editing, setEditing] = useState<{ id: string; value: string } | null>(
    null
  );

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

  const onError = (err: unknown) =>
    setError(err instanceof Error ? err.message : 'Request failed');
  const onSettled = () => queryClient.invalidateQueries({ queryKey });

  const createToken = useMutation({
    mutationFn: (data: { owner: string; value: string }) =>
      client().createGitHubOwnerToken(data),
    onSuccess: () => {
      setNewOwner('');
      setNewValue('');
    },
    onError,
    onSettled,
  });
  const updateToken = useMutation({
    mutationFn: (data: { id: string; value: string }) =>
      client().updateGitHubOwnerToken(data.id, { value: data.value }),
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
    createToken.mutate({ owner, value: submittedValue(newValue) });
  };

  const handleStartEdit = (token: GitHubOwnerToken) => {
    setError(null);
    setEditing({ id: token.id, value: token.reference ?? '' });
  };

  const handleSaveEdit = () => {
    if (!editing) return;
    setError(null);
    updateToken.mutate({
      id: editing.id,
      value: submittedValue(editing.value),
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
      description="One fine-grained personal access token per GitHub organization or user. Workspace sessions on this machine use the matching owner's token for gh commands (by the repository each command targets) and for HTTPS git operations; other owners keep their existing authentication. Enter a token or an op://vault/item/field reference. Values are encrypted at rest; tokens are never shown again, while references stay visible. Changes apply to newly started processes."
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
                    <Input
                      autoFocus
                      type={draftInputType(editing.value)}
                      placeholder="New token or op://vault/item/field"
                      value={editing.value}
                      onChange={(e) =>
                        setEditing({
                          id: token.id,
                          value: unquoteDraft(e.target.value),
                        })
                      }
                      className="mt-2"
                      autoComplete="off"
                    />
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
                          editing.value.trim().length === 0
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

      <div className="border-t border-border pt-4 space-y-3">
        <SettingsField label="Add organization token">
          <div className="flex flex-col gap-2 md:flex-row">
            <Input
              placeholder="GitHub owner"
              value={newOwner}
              onChange={(e) => setNewOwner(e.target.value)}
              className="md:w-48 font-mono"
              autoComplete="off"
            />
            <Input
              type={draftInputType(newValue)}
              placeholder="token or op://vault/item/field"
              value={newValue}
              onChange={(e) => setNewValue(unquoteDraft(e.target.value))}
              className="flex-1"
              autoComplete="off"
            />
            <PrimaryButton
              variant="secondary"
              value="Add"
              onClick={handleAdd}
              disabled={
                createToken.isPending ||
                machineClient == null ||
                newOwner.trim().length === 0 ||
                newValue.trim().length === 0
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
