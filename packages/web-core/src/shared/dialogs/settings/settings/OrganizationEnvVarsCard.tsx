import { useState } from 'react';
import {
  SpinnerIcon,
  PlusIcon,
  TrashIcon,
  PencilSimpleIcon,
} from '@phosphor-icons/react';
import { Input } from '@vibe/ui/components/Input';
import { Button } from '@vibe/ui/components/Button';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import {
  useOrganizationEnvVars,
  useOrganizationEnvVarMutations,
} from '@/shared/hooks/useOrganizationEnvVars';
import type { OrganizationEnvVar } from 'shared/types';
import { cn } from '@/shared/lib/utils';
import { SettingsCard, SettingsField } from './SettingsComponents';

const NAME_PATTERN = /^[A-Za-z_][A-Za-z0-9_]*$/;

interface Props {
  organizationId: string;
}

export function OrganizationEnvVarsCard({ organizationId }: Props) {
  const [error, setError] = useState<string | null>(null);
  const [newName, setNewName] = useState('');
  const [newValue, setNewValue] = useState('');
  const [editing, setEditing] = useState<{ id: string; value: string } | null>(
    null
  );

  const { data: envVars = [], isLoading } = useOrganizationEnvVars({
    organizationId,
    enabled: true,
  });

  const { createEnvVar, updateEnvVar, deleteEnvVar } =
    useOrganizationEnvVarMutations(organizationId, {
      onError: (err) =>
        setError(err instanceof Error ? err.message : 'Request failed'),
    });

  const resetAddForm = () => {
    setNewName('');
    setNewValue('');
  };

  const handleAdd = () => {
    setError(null);
    const name = newName.trim();
    if (!NAME_PATTERN.test(name)) {
      setError('Name must match [A-Za-z_][A-Za-z0-9_]*');
      return;
    }
    if (envVars.some((v) => v.name === name)) {
      setError('An env var with this name already exists');
      return;
    }
    createEnvVar.mutate({ name, value: newValue }, { onSuccess: resetAddForm });
  };

  const handleStartEdit = (envVar: OrganizationEnvVar) => {
    setError(null);
    setEditing({ id: envVar.id, value: '' });
  };

  const handleCancelEdit = () => {
    setEditing(null);
  };

  const handleSaveEdit = () => {
    if (!editing) return;
    setError(null);
    updateEnvVar.mutate(
      { id: editing.id, value: editing.value },
      { onSuccess: () => setEditing(null) }
    );
  };

  const handleDelete = (envVar: OrganizationEnvVar) => {
    const confirmed = window.confirm(
      `Delete env var "${envVar.name}"? This cannot be undone.`
    );
    if (!confirmed) return;
    setError(null);
    deleteEnvVar.mutate(envVar.id);
  };

  return (
    <SettingsCard
      title="Environment variables"
      description="Variables stored encrypted at rest and scoped to this organization."
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
      ) : envVars.length === 0 ? (
        <div className="text-sm text-low">No env vars set.</div>
      ) : (
        <div className="space-y-2">
          {envVars.map((envVar) => {
            const isEditing = editing?.id === envVar.id;
            return (
              <div
                key={envVar.id}
                className="border border-border rounded-sm p-3 flex items-center gap-3"
              >
                <div className="flex-1 min-w-0">
                  <div className="font-mono text-sm text-high truncate">
                    {envVar.name}
                  </div>
                  {isEditing ? (
                    <Input
                      autoFocus
                      type="password"
                      placeholder="New value"
                      value={editing.value}
                      onChange={(e) =>
                        setEditing({ id: envVar.id, value: e.target.value })
                      }
                      className="mt-2"
                    />
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
                        onClick={handleCancelEdit}
                        disabled={updateEnvVar.isPending}
                      >
                        Cancel
                      </Button>
                      <PrimaryButton
                        variant="secondary"
                        value="Save"
                        onClick={handleSaveEdit}
                        disabled={updateEnvVar.isPending}
                      />
                    </>
                  ) : (
                    <>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => handleStartEdit(envVar)}
                        aria-label={`Edit ${envVar.name}`}
                      >
                        <PencilSimpleIcon
                          className="size-icon-xs"
                          weight="bold"
                        />
                      </Button>
                      <button
                        type="button"
                        onClick={() => handleDelete(envVar)}
                        disabled={deleteEnvVar.isPending}
                        aria-label={`Delete ${envVar.name}`}
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
        <SettingsField label="Add new variable">
          <div className="flex flex-col gap-2 md:flex-row">
            <Input
              placeholder="NAME"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              className="md:w-48 font-mono"
              autoComplete="off"
            />
            <Input
              type="password"
              placeholder="value"
              value={newValue}
              onChange={(e) => setNewValue(e.target.value)}
              className="flex-1"
              autoComplete="off"
            />
            <PrimaryButton
              variant="secondary"
              value="Add"
              onClick={handleAdd}
              disabled={
                createEnvVar.isPending ||
                newName.trim().length === 0 ||
                newValue.length === 0
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
