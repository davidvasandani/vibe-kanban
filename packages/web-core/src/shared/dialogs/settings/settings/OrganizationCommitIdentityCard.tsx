import { useState } from 'react';
import { SpinnerIcon } from '@phosphor-icons/react';
import { Input } from '@vibe/ui/components/Input';
import { Button } from '@vibe/ui/components/Button';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { Textarea } from '@vibe/ui/components/Textarea';
import {
  useOrganizationCommitIdentity,
  useOrganizationCommitIdentityMutations,
} from '@/shared/hooks/useOrganizationCommitIdentity';
import type { OrganizationCommitIdentity } from 'shared/types';
import { SettingsCard, SettingsField } from './SettingsComponents';

const EMAIL_PATTERN = /^[^\s<>@,"]+@[^\s<>@,"]+$/;

/** One address per line; commas and blank lines are tolerated. */
export const parseAllowedEmails = (value: string) =>
  value
    .split(/[\n,]/)
    .map((email) => email.trim())
    .filter((email) => email.length > 0);

interface Props {
  organizationId: string;
}

export function OrganizationCommitIdentityCard({ organizationId }: Props) {
  const { data: identity, isLoading } =
    useOrganizationCommitIdentity(organizationId);

  return (
    <SettingsCard
      title="Commit identity"
      description="The git name and email every workspace in this organization commits as. It is written into each workspace worktree's own git config when the workspace is created and before every agent turn, on every machine, and a pre-commit check rejects any other author or committer email unless it is listed below."
    >
      {isLoading ? (
        <div className="flex items-center justify-center py-4 gap-2">
          <SpinnerIcon className="size-icon-sm animate-spin" />
          <span className="text-sm text-low">Loading…</span>
        </div>
      ) : (
        // Remount the form when the saved identity changes so the draft
        // always starts from what the server holds.
        <CommitIdentityForm
          key={identity?.updated_at ?? 'none'}
          organizationId={organizationId}
          identity={identity ?? null}
        />
      )}
    </SettingsCard>
  );
}

function CommitIdentityForm({
  organizationId,
  identity,
}: {
  organizationId: string;
  identity: OrganizationCommitIdentity | null;
}) {
  const [error, setError] = useState<string | null>(null);
  const [name, setName] = useState(identity?.name ?? '');
  const [email, setEmail] = useState(identity?.email ?? '');
  const [allowed, setAllowed] = useState(
    identity?.allowed_emails.join('\n') ?? ''
  );

  const { saveCommitIdentity, removeCommitIdentity } =
    useOrganizationCommitIdentityMutations(organizationId, {
      onError: (err) =>
        setError(err instanceof Error ? err.message : 'Request failed'),
    });
  const pending =
    saveCommitIdentity.isPending || removeCommitIdentity.isPending;

  const handleSave = () => {
    setError(null);
    const allowedEmails = parseAllowedEmails(allowed);
    const invalid = [email.trim(), ...allowedEmails].find(
      (value) => !EMAIL_PATTERN.test(value)
    );
    if (name.trim().length === 0) {
      setError('Name is required');
      return;
    }
    if (invalid !== undefined) {
      setError(`"${invalid}" is not a valid email address`);
      return;
    }
    saveCommitIdentity.mutate({
      name: name.trim(),
      email: email.trim(),
      allowed_emails: allowedEmails,
    });
  };

  const handleRemove = () => {
    const confirmed = window.confirm(
      'Stop pinning a commit identity for this organization? Workspaces stop enforcing it on their next turn.'
    );
    if (!confirmed) return;
    setError(null);
    removeCommitIdentity.mutate();
  };

  return (
    <div className="space-y-3">
      {error && (
        <div className="bg-error/10 border border-error/50 rounded-sm p-3 text-error text-sm">
          {error}
        </div>
      )}
      <div className="flex flex-col gap-2 md:flex-row">
        <SettingsField label="Name">
          <Input
            placeholder="Jane Doe"
            value={name}
            onChange={(e) => setName(e.target.value)}
            autoComplete="off"
          />
        </SettingsField>
        <SettingsField label="Email">
          <Input
            placeholder="jane@example.com"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            autoComplete="off"
          />
        </SettingsField>
      </div>
      <SettingsField
        label="Also allow these author and committer emails"
        description="Agent and bot identities, one per line, e.g. noreply@anthropic.com or noreply@vibekanban.com."
      >
        <Textarea
          placeholder={'noreply@anthropic.com\nnoreply@vibekanban.com'}
          value={allowed}
          onChange={(e) => setAllowed(e.target.value)}
          className="font-mono"
        />
      </SettingsField>
      <div className="flex items-center justify-end gap-2">
        {identity && (
          <Button
            variant="ghost"
            size="sm"
            onClick={handleRemove}
            disabled={pending}
          >
            Remove
          </Button>
        )}
        <PrimaryButton
          variant="secondary"
          value="Save"
          onClick={handleSave}
          disabled={
            pending || name.trim().length === 0 || email.trim().length === 0
          }
        />
      </div>
    </div>
  );
}
