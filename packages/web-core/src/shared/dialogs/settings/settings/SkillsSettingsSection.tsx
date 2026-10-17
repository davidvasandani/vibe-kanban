import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  ArrowSquareOutIcon,
  PlusIcon,
  SpinnerIcon,
  TrashIcon,
} from '@phosphor-icons/react';
import { Button } from '@vibe/ui/components/Button';
import type { ManagedSkill, SkillChange } from '@/shared/lib/machineClient';
import { SettingsCard } from './SettingsComponents';
import { useSettingsMachineClient } from './SettingsHostContext';
import { useSettingsDirty } from './SettingsDirtyContext';

type EditableSkill = ManagedSkill & { originalName?: string };

const emptySkill = (): EditableSkill => ({
  name: '',
  description: '',
  instructions: '# Instructions\n\n',
});

export function SkillsSettingsSection() {
  const { t } = useTranslation(['settings']);
  const machineClient = useSettingsMachineClient();
  const { setDirty } = useSettingsDirty();
  const [original, setOriginal] = useState<ManagedSkill[]>([]);
  const [skills, setSkills] = useState<EditableSkill[]>([]);
  const [deleted, setDeleted] = useState<ManagedSkill[]>([]);
  const [selected, setSelected] = useState(0);
  const [available, setAvailable] = useState<boolean | null>(null);
  const [baseBranch, setBaseBranch] = useState('main');
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [proposalTitle, setProposalTitle] = useState('Update managed skills');
  const [proposalBody, setProposalBody] = useState(
    'Created from Vibe Kanban Settings → Skills.'
  );
  const [pullRequestUrl, setPullRequestUrl] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!machineClient) return;
    setError(null);
    try {
      const catalog = await machineClient.loadSkills();
      setAvailable(catalog.available);
      setBaseBranch(catalog.baseBranch);
      setOriginal(catalog.skills);
      setSkills(
        catalog.skills.map((skill) => ({
          ...skill,
          originalName: skill.name,
        }))
      );
      setDeleted([]);
      setSelected(0);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      setAvailable(false);
    }
  }, [machineClient]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const changes = useMemo<SkillChange[]>(() => {
    const originalByName = new Map(
      original.map((skill) => [skill.name, skill])
    );
    const upserts = skills
      .filter((skill) => {
        const before = skill.originalName
          ? originalByName.get(skill.originalName)
          : undefined;
        const draft = {
          name: skill.name,
          description: skill.description,
          instructions: skill.instructions,
        };
        return !before || JSON.stringify(before) !== JSON.stringify(draft);
      })
      .map((skill) => {
        const { originalName, ...draft } = skill;
        const recreated = deleted.find((item) => item.name === draft.name);
        return {
          operation: 'upsert' as const,
          ...draft,
          previous: originalName ? originalByName.get(originalName) : recreated,
        };
      });
    const upsertNames = new Set(upserts.map((change) => change.name));
    return [
      ...upserts,
      ...deleted
        .filter((previous) => !upsertNames.has(previous.name))
        .map((previous) => ({
          operation: 'delete' as const,
          name: previous.name,
          previous,
        })),
    ];
  }, [deleted, original, skills]);

  useEffect(() => {
    setDirty('skills', changes.length > 0);
    return () => setDirty('skills', false);
  }, [changes.length, setDirty]);

  const current = skills[selected];
  const updateCurrent = (patch: Partial<ManagedSkill>) => {
    setSkills((items) =>
      items.map((skill, index) =>
        index === selected ? { ...skill, ...patch } : skill
      )
    );
  };

  const addSkill = () => {
    setSkills((items) => [...items, emptySkill()]);
    setSelected(skills.length);
    setPullRequestUrl(null);
  };

  const removeCurrent = () => {
    if (!current) return;
    if (current.originalName) {
      const previous = original.find(
        (skill) => skill.name === current.originalName
      );
      if (previous) {
        setDeleted((items) => [...items, previous]);
      }
    }
    setSkills((items) => items.filter((_, index) => index !== selected));
    setSelected(Math.max(0, selected - 1));
    setPullRequestUrl(null);
  };

  const submit = async () => {
    if (!machineClient || changes.length === 0) return;
    setSubmitting(true);
    setError(null);
    try {
      const result = await machineClient.createSkillProposal({
        title: proposalTitle,
        body: proposalBody,
        changes,
      });
      setPullRequestUrl(result.pullRequestUrl);
      await refresh();
      setPullRequestUrl(result.pullRequestUrl);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="space-y-4">
      <SettingsCard
        title={t('settings.skills.title')}
        description={t('settings.skills.description', { baseBranch })}
        headerAction={
          <Button
            size="sm"
            variant="secondary"
            disabled={available !== true}
            onClick={addSkill}
          >
            <PlusIcon className="size-icon-sm" />
            {t('settings.skills.add')}
          </Button>
        }
      >
        {available === null && (
          <div className="flex items-center gap-2 text-sm text-low">
            <SpinnerIcon className="size-icon-sm animate-spin" />
            {t('settings.skills.loading')}
          </div>
        )}
        {available === false && !error && (
          <p className="text-sm text-low">{t('settings.skills.unavailable')}</p>
        )}
        {error && <p className="text-sm text-error">{error}</p>}
        {available && (
          <div className="grid min-h-96 grid-cols-[15rem_1fr] overflow-hidden rounded-sm border border-border">
            <div className="border-r border-border bg-secondary/40 p-2">
              <div className="space-y-1">
                {skills.map((skill, index) => (
                  <button
                    key={`${skill.name}-${index}`}
                    type="button"
                    onClick={() => setSelected(index)}
                    className={`w-full rounded-sm px-3 py-2 text-left text-sm ${
                      selected === index
                        ? 'bg-brand/10 text-brand'
                        : 'text-normal hover:bg-primary/10'
                    }`}
                  >
                    {skill.name || t('settings.skills.untitled')}
                  </button>
                ))}
              </div>
            </div>
            {current ? (
              <div className="space-y-4 p-4">
                <div>
                  <label className="mb-1 block text-sm font-medium text-normal">
                    {t('settings.skills.fields.name')}
                  </label>
                  <input
                    value={current.name}
                    onChange={(event) =>
                      updateCurrent({ name: event.target.value })
                    }
                    readOnly={Boolean(current.originalName)}
                    className="w-full rounded-sm border border-border bg-secondary px-3 py-2 text-sm text-normal focus:outline-none focus:ring-1 focus:ring-brand"
                    placeholder="my-skill"
                  />
                </div>
                <div>
                  <label className="mb-1 block text-sm font-medium text-normal">
                    {t('settings.skills.fields.description')}
                  </label>
                  <textarea
                    value={current.description}
                    onChange={(event) =>
                      updateCurrent({ description: event.target.value })
                    }
                    rows={3}
                    className="w-full resize-y rounded-sm border border-border bg-secondary px-3 py-2 text-sm text-normal focus:outline-none focus:ring-1 focus:ring-brand"
                  />
                </div>
                <div>
                  <label className="mb-1 block text-sm font-medium text-normal">
                    {t('settings.skills.fields.instructions')}
                  </label>
                  <textarea
                    value={current.instructions}
                    onChange={(event) =>
                      updateCurrent({ instructions: event.target.value })
                    }
                    rows={14}
                    className="w-full resize-y rounded-sm border border-border bg-secondary px-3 py-2 font-ibm-plex-mono text-sm text-normal focus:outline-none focus:ring-1 focus:ring-brand"
                  />
                </div>
                <Button size="sm" variant="secondary" onClick={removeCurrent}>
                  <TrashIcon className="size-icon-sm" />
                  {t('settings.skills.delete')}
                </Button>
              </div>
            ) : (
              <div className="flex items-center justify-center p-6 text-sm text-low">
                {t('settings.skills.empty')}
              </div>
            )}
          </div>
        )}
      </SettingsCard>

      {available && (
        <SettingsCard
          title={t('settings.skills.proposal.title')}
          description={t('settings.skills.proposal.description')}
        >
          <div className="space-y-3">
            <input
              value={proposalTitle}
              onChange={(event) => setProposalTitle(event.target.value)}
              className="w-full rounded-sm border border-border bg-secondary px-3 py-2 text-sm text-normal"
            />
            <textarea
              value={proposalBody}
              onChange={(event) => setProposalBody(event.target.value)}
              rows={3}
              className="w-full resize-y rounded-sm border border-border bg-secondary px-3 py-2 text-sm text-normal"
            />
            <div className="flex items-center gap-3">
              <Button
                disabled={submitting || changes.length === 0}
                onClick={() => void submit()}
              >
                {submitting && (
                  <SpinnerIcon className="size-icon-sm animate-spin" />
                )}
                {t('settings.skills.proposal.submit', {
                  count: changes.length,
                })}
              </Button>
              {pullRequestUrl && (
                <a
                  href={pullRequestUrl}
                  target="_blank"
                  rel="noreferrer"
                  className="flex items-center gap-1 text-sm text-brand hover:underline"
                >
                  {t('settings.skills.proposal.open')}
                  <ArrowSquareOutIcon className="size-icon-sm" />
                </a>
              )}
            </div>
          </div>
        </SettingsCard>
      )}
    </div>
  );
}
