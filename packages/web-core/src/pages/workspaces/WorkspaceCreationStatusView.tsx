import { useTranslation } from 'react-i18next';
import type { Workspace } from 'shared/types';

export function WorkspaceCreationStatusView({
  workspace,
}: {
  workspace: Workspace;
}) {
  const { t } = useTranslation('common');

  if (
    workspace.creation_status === 'queued' ||
    workspace.creation_status === 'running'
  ) {
    return (
      <div className="flex h-full items-center justify-center p-6">
        <div className="max-w-md text-center" role="status">
          <div className="mx-auto mb-4 h-6 w-6 animate-spin rounded-full border-2 border-muted-foreground/30 border-t-foreground" />
          <h2 className="text-lg font-medium">
            {t('workspaceCreation.creatingTitle')}
          </h2>
          <p className="mt-2 text-sm text-muted-foreground">
            {t('workspaceCreation.creatingBody')}
          </p>
        </div>
      </div>
    );
  }

  if (workspace.creation_status === 'failed') {
    return (
      <div className="flex h-full items-center justify-center p-6">
        <div className="max-w-md text-center" role="alert">
          <h2 className="text-lg font-medium">
            {t('workspaceCreation.failedTitle')}
          </h2>
          <p className="mt-2 text-sm text-muted-foreground">
            {workspace.creation_error ?? t('workspaceCreation.failedFallback')}
          </p>
        </div>
      </div>
    );
  }

  return null;
}
