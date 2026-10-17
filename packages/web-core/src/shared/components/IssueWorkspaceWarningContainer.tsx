import { useMemo, useState } from 'react';
import {
  PROJECT_PULL_REQUESTS_SHAPE,
  PROJECT_WORKSPACES_SHAPE,
} from 'shared/remote-types';
import { IssueWorkspaceWarning } from '@vibe/ui/components/IssueWorkspaceWarning';
import { useShape } from '@/shared/integrations/electric/hooks';
import { useWorkspaceContext } from '@/shared/hooks/useWorkspaceContext';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { useAuth } from '@/shared/hooks/auth/useAuth';
import {
  getIssueWorkspaceAdvisory,
  issueWorkspaceAdvisoryKey,
} from '@/shared/lib/issueWorkspaceAdvisory';

export function IssueWorkspaceWarningContainer({
  projectId,
  issueId,
  className,
}: {
  projectId: string;
  issueId: string;
  className?: string;
}) {
  const params = useMemo(() => ({ project_id: projectId }), [projectId]);
  const { data: workspaces } = useShape(PROJECT_WORKSPACES_SHAPE, params);
  const { data: pullRequests } = useShape(PROJECT_PULL_REQUESTS_SHAPE, params);
  const { activeWorkspaces, archivedWorkspaces } = useWorkspaceContext();
  const { userId } = useAuth();
  const navigation = useAppNavigation();
  const [dismissedKey, setDismissedKey] = useState<string | null>(null);
  const siblings = getIssueWorkspaceAdvisory({
    projectId,
    issueId,
    workspaces,
    pullRequests,
    localWorkspaces: [...activeWorkspaces, ...archivedWorkspaces],
    userId,
  });
  const key = issueWorkspaceAdvisoryKey(projectId, issueId, siblings);
  if (key === dismissedKey) return null;
  return (
    <IssueWorkspaceWarning
      workspaces={siblings}
      className={className}
      onDismiss={() => setDismissedKey(key)}
      onOpen={(workspaceId) =>
        navigation.goToProjectIssueWorkspace(projectId, issueId, workspaceId)
      }
    />
  );
}
