import { useEffect, useMemo, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { SpinnerIcon } from '@phosphor-icons/react';
import type { ChngSettings, UpdateChngSettingsRequest } from 'shared/types';
import {
  SettingsCard,
  SettingsField,
  SettingsInput,
  SettingsSaveBar,
  SettingsTextarea,
} from './SettingsComponents';
import { useSettingsDirty } from './SettingsDirtyContext';
import { useSettingsMachineClient } from './SettingsHostContext';
import { normalizeSecretReference } from './secretReference';

type Draft = {
  jira_base_url: string;
  jira_email: string;
  jql: string;
  hashmal_mcp_url: string;
};

/** A token field: a new value typed, or a request to clear the stored one. */
type TokenDraft = { value: string; clear: boolean };

const EMPTY_TOKEN: TokenDraft = { value: '', clear: false };

function draftFrom(settings: ChngSettings): Draft {
  return {
    jira_base_url: settings.jira_base_url,
    jira_email: settings.jira_email,
    jql: settings.jql,
    hashmal_mcp_url: settings.hashmal_mcp_url,
  };
}

const sameUrl = (a: string, b: string) =>
  a.trim().replace(/\/+$/, '') === b.trim().replace(/\/+$/, '');

/** Absent keeps the stored token, '' clears it, anything else replaces it. */
function tokenField(token: TokenDraft): string | undefined {
  if (token.clear) return '';
  if (!token.value.trim()) return undefined;
  return normalizeSecretReference(token.value) ?? token.value.trim();
}

/**
 * A saved token is only sent to the URL it was saved with, so moving the URL
 * needs the token again (the server enforces this too).
 */
function needsToken(
  savedUrl: string,
  draftUrl: string,
  hasToken: boolean,
  token: TokenDraft
): boolean {
  return (
    hasToken &&
    draftUrl.trim() !== '' &&
    !sameUrl(savedUrl, draftUrl) &&
    !token.clear &&
    !token.value.trim()
  );
}

function TokenInput({
  label,
  hasToken,
  reference,
  token,
  onChange,
  error,
  description,
}: {
  label: string;
  hasToken: boolean;
  reference?: string;
  token: TokenDraft;
  onChange: (token: TokenDraft) => void;
  error?: string | null;
  description: string;
}) {
  const status = token.clear
    ? 'Will be removed on save'
    : hasToken
      ? reference
        ? `Stored: ${reference}`
        : 'Stored (hidden)'
      : 'Not set';
  return (
    <SettingsField label={label} error={error} description={description}>
      <div className="flex items-center gap-2">
        <input
          type={
            normalizeSecretReference(token.value) === null ? 'password' : 'text'
          }
          aria-label={label}
          value={token.value}
          onChange={(e) => onChange({ value: e.target.value, clear: false })}
          placeholder={
            hasToken
              ? 'Leave empty to keep the stored token'
              : 'op://… or token'
          }
          autoComplete="off"
          className="w-full bg-secondary border border-border rounded-sm px-base py-half text-sm text-high placeholder:text-low placeholder:opacity-80 focus:outline-none focus:ring-1 focus:ring-brand"
        />
        {hasToken && (
          <button
            type="button"
            onClick={() => onChange({ value: '', clear: !token.clear })}
            className="shrink-0 text-sm text-low hover:text-normal cursor-pointer"
          >
            {token.clear ? 'Keep' : 'Remove'}
          </button>
        )}
      </div>
      <p className="text-xs text-low">{status}</p>
    </SettingsField>
  );
}

export function ChngSettingsSection() {
  const machineClient = useSettingsMachineClient();
  const queryClient = useQueryClient();
  const { setDirty } = useSettingsDirty();
  const queryKey = [
    'chng-settings',
    ...(machineClient?.queryScopeKey ?? ['machine', 'unselected']),
  ] as const;

  const settings = useQuery({
    queryKey,
    queryFn: () => {
      if (!machineClient) throw new Error('Machine client is required');
      return machineClient.getChngSettings();
    },
    enabled: machineClient != null,
  });

  const [draft, setDraft] = useState<Draft | null>(null);
  const [jiraToken, setJiraToken] = useState<TokenDraft>(EMPTY_TOKEN);
  const [hashmalToken, setHashmalToken] = useState<TokenDraft>(EMPTY_TOKEN);
  const [error, setError] = useState<string | null>(null);

  const saved = settings.data;
  useEffect(() => {
    if (saved && draft === null) setDraft(draftFrom(saved));
  }, [saved, draft]);

  const isDirty = useMemo(() => {
    if (!saved || !draft) return false;
    const base = draftFrom(saved);
    return (
      (Object.keys(base) as (keyof Draft)[]).some(
        (key) => base[key] !== draft[key]
      ) ||
      tokenField(jiraToken) !== undefined ||
      tokenField(hashmalToken) !== undefined
    );
  }, [saved, draft, jiraToken, hashmalToken]);

  useEffect(() => {
    setDirty('chng', isDirty);
    return () => setDirty('chng', false);
  }, [isDirty, setDirty]);

  const save = useMutation({
    mutationFn: (request: UpdateChngSettingsRequest) => {
      if (!machineClient) throw new Error('Machine client is required');
      return machineClient.updateChngSettings(request);
    },
    onSuccess: (next) => {
      queryClient.setQueryData(queryKey, next);
      setDraft(draftFrom(next));
      setJiraToken(EMPTY_TOKEN);
      setHashmalToken(EMPTY_TOKEN);
      setError(null);
      void queryClient.invalidateQueries({ queryKey: ['chng'] });
    },
    onError: (err: unknown) =>
      setError(err instanceof Error ? err.message : 'Saving failed'),
  });

  if (settings.isError) {
    return (
      <p className="text-sm text-error">
        {settings.error instanceof Error
          ? settings.error.message
          : 'Could not load CHNG settings'}
      </p>
    );
  }
  if (!machineClient || settings.isPending || !draft || !saved) {
    return (
      <div className="flex items-center gap-2 text-sm text-low">
        <SpinnerIcon className="size-icon-sm animate-spin" />
        Loading CHNG settings…
      </div>
    );
  }

  const update = (patch: Partial<Draft>) =>
    setDraft((prev) => (prev ? { ...prev, ...patch } : prev));
  const jiraNeedsToken = needsToken(
    saved.jira_base_url,
    draft.jira_base_url,
    saved.has_jira_token,
    jiraToken
  );
  const hashmalNeedsToken = needsToken(
    saved.hashmal_mcp_url,
    draft.hashmal_mcp_url,
    saved.has_hashmal_token,
    hashmalToken
  );
  const reenter =
    'Re-enter the token: it is only sent to the URL it was saved with';

  const handleSave = () => {
    setError(null);
    save.mutate({
      ...draft,
      jira_token: tokenField(jiraToken),
      hashmal_token: tokenField(hashmalToken),
    });
  };

  return (
    <div className="space-y-6">
      <SettingsCard
        title="Jira search"
        description="The CHNG list in the left rail shows the tickets this Jira Cloud search returns. Tokens can be a literal API token or a 1Password reference."
      >
        <SettingsField label="Jira site URL">
          <SettingsInput
            value={draft.jira_base_url}
            onChange={(jira_base_url) => update({ jira_base_url })}
            placeholder="https://your-site.atlassian.net"
          />
        </SettingsField>
        <SettingsField label="Account email">
          <SettingsInput
            value={draft.jira_email}
            onChange={(jira_email) => update({ jira_email })}
            placeholder="you@example.com"
          />
        </SettingsField>
        <TokenInput
          label="Jira API token"
          hasToken={saved.has_jira_token}
          reference={saved.jira_token_reference}
          token={jiraToken}
          onChange={setJiraToken}
          error={jiraNeedsToken ? reenter : null}
          description="Create one at id.atlassian.com → Security → API tokens."
        />
        <SettingsField
          label="JQL search"
          description={
            draft.jql.trim() === saved.default_jql
              ? 'The default: CHNG tickets you reported.'
              : 'currentUser() is the account that owns the API token.'
          }
        >
          <SettingsTextarea
            value={draft.jql}
            onChange={(jql) => update({ jql })}
            rows={3}
            monospace
          />
          {draft.jql.trim() !== saved.default_jql && (
            <button
              type="button"
              onClick={() => update({ jql: saved.default_jql })}
              className="text-sm text-low hover:text-normal cursor-pointer"
            >
              Reset to default
            </button>
          )}
        </SettingsField>
      </SettingsCard>

      <SettingsCard
        title="Hashmal"
        description="Before a session starts from a CHNG ticket, Vibe Kanban asks Hashmal which earlier session worked it, so you can continue that one. Use Hashmal's MCP URL and a token from `python manage.py mcp_token create`."
      >
        <SettingsField label="Hashmal MCP URL">
          <SettingsInput
            value={draft.hashmal_mcp_url}
            onChange={(hashmal_mcp_url) => update({ hashmal_mcp_url })}
            placeholder="https://tickets.example.com/mcp"
          />
        </SettingsField>
        <TokenInput
          label="Hashmal token"
          hasToken={saved.has_hashmal_token}
          reference={saved.hashmal_token_reference}
          token={hashmalToken}
          onChange={setHashmalToken}
          error={hashmalNeedsToken ? reenter : null}
          description="A Hashmal MCP bearer token (hsm_…)."
        />
      </SettingsCard>

      {error && <p className="text-sm text-error">{error}</p>}

      <SettingsSaveBar
        show={isDirty}
        saving={save.isPending}
        saveDisabled={jiraNeedsToken || hashmalNeedsToken}
        onSave={handleSave}
        onDiscard={() => {
          setDraft(draftFrom(saved));
          setJiraToken(EMPTY_TOKEN);
          setHashmalToken(EMPTY_TOKEN);
          setError(null);
        }}
      />
    </div>
  );
}
