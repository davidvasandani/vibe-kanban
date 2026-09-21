import { useTranslation } from 'react-i18next';
import { cn } from '../lib/cn';
import { PrimaryButton } from './PrimaryButton';

export interface IssueWorkspaceWarningRow {
  id: string;
  name: string;
  branch: string | null;
  localWorkspaceId: string | null;
  activity: 'creating' | 'running' | 'idle' | 'unknown';
  hasOpenPr: boolean;
  hasChanges: boolean;
}

export function IssueWorkspaceWarning({
  workspaces,
  onDismiss,
  onOpen,
  className,
}: {
  workspaces: IssueWorkspaceWarningRow[];
  onDismiss: () => void;
  onOpen: (localWorkspaceId: string) => void;
  className?: string;
}) {
  const { t } = useTranslation('common');
  if (workspaces.length === 0) return null;
  return (
    <aside
      role="status"
      className={cn(
        'rounded border border-brand bg-secondary p-base text-normal',
        className
      )}
    >
      <div className="flex items-start justify-between gap-base">
        <div>
          <p className="font-medium text-high">
            {t('workspaces.duplicateWarning.title')}
          </p>
          <p>{t('workspaces.duplicateWarning.description')}</p>
        </div>
        <PrimaryButton variant="tertiary" onClick={onDismiss}>
          {t('workspaces.duplicateWarning.dismiss')}
        </PrimaryButton>
      </div>
      <ul className="mt-base max-h-48 overflow-y-auto space-y-half">
        {workspaces.map((workspace) => (
          <li
            key={workspace.id}
            className="flex flex-wrap items-center justify-between gap-half"
          >
            <div className="min-w-0 break-words">
              <p className="font-medium">{workspace.name}</p>
              <p className="font-ibm-plex-mono break-all">
                {workspace.branch ??
                  t('workspaces.duplicateWarning.branchUnavailable')}
              </p>
              <p>
                {t(
                  `workspaces.duplicateWarning.activity.${workspace.activity}`
                )}
              </p>
              {(workspace.hasOpenPr || workspace.hasChanges) && (
                <p className="font-medium text-brand">
                  {workspace.hasOpenPr
                    ? t('workspaces.duplicateWarning.openPr')
                    : t('workspaces.duplicateWarning.changes')}
                </p>
              )}
            </div>
            {workspace.localWorkspaceId && (
              <PrimaryButton
                variant="tertiary"
                onClick={() => onOpen(workspace.localWorkspaceId!)}
              >
                {t('workspaces.duplicateWarning.open')}
              </PrimaryButton>
            )}
          </li>
        ))}
      </ul>
    </aside>
  );
}
