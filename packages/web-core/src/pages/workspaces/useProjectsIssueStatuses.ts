import { useEffect, useMemo, useState } from 'react';
import { createShapeCollection } from '@/shared/lib/electric/collections';
import {
  PROJECT_ISSUES_SHAPE,
  PROJECT_PROJECT_STATUSES_SHAPE,
  type Issue,
  type ProjectStatus,
} from 'shared/remote-types';
import { useAuth } from '@/shared/hooks/auth/useAuth';
import { normalizeStatusName } from './workspaceSidebarFilters';

interface UseProjectsIssueStatusesOptions {
  enabled?: boolean;
}

/**
 * Syncs issues and statuses for several remote projects and exposes each
 * issue's normalized status name. Like useAllOrganizationProjects, it uses the
 * raw collection API so the number of projects can vary without calling
 * useShape in a loop. Collections are cached, so projects already synced by
 * other views are reused.
 */
export function useProjectsIssueStatuses(
  projectIds: string[],
  options: UseProjectsIssueStatusesOptions = {}
) {
  const { enabled = true } = options;
  const { isSignedIn } = useAuth();

  // Stable key so a new array with the same IDs doesn't resubscribe
  const projectIdsKey = useMemo(
    () => Array.from(new Set(projectIds)).sort().join(','),
    [projectIds]
  );

  const [statuses, setStatuses] = useState<ProjectStatus[]>([]);
  const [issues, setIssues] = useState<Issue[]>([]);

  useEffect(() => {
    const ids = projectIdsKey ? projectIdsKey.split(',') : [];
    if (!enabled || !isSignedIn || ids.length === 0) {
      setStatuses([]);
      setIssues([]);
      return;
    }

    const subscriptions: { unsubscribe: () => void }[] = [];
    const statusesByProject = new Map<string, ProjectStatus[]>();
    const issuesByProject = new Map<string, Issue[]>();

    for (const projectId of ids) {
      const params = { project_id: projectId };

      const statusCollection = createShapeCollection(
        PROJECT_PROJECT_STATUSES_SHAPE,
        params
      );
      subscriptions.push(
        statusCollection.subscribeChanges(
          () => {
            statusesByProject.set(
              projectId,
              statusCollection.toArray as unknown as ProjectStatus[]
            );
            setStatuses(Array.from(statusesByProject.values()).flat());
          },
          { includeInitialState: true }
        )
      );

      const issueCollection = createShapeCollection(
        PROJECT_ISSUES_SHAPE,
        params
      );
      subscriptions.push(
        issueCollection.subscribeChanges(
          () => {
            issuesByProject.set(
              projectId,
              issueCollection.toArray as unknown as Issue[]
            );
            setIssues(Array.from(issuesByProject.values()).flat());
          },
          { includeInitialState: true }
        )
      );
    }

    return () => {
      subscriptions.forEach((s) => s.unsubscribe());
    };
  }, [enabled, isSignedIn, projectIdsKey]);

  const issueStatusNameById = useMemo(() => {
    const statusNameById = new Map<string, string>();
    for (const status of statuses) {
      statusNameById.set(status.id, normalizeStatusName(status.name));
    }

    const map = new Map<string, string>();
    for (const issue of issues) {
      const name = statusNameById.get(issue.status_id);
      if (name !== undefined) map.set(issue.id, name);
    }
    return map;
  }, [statuses, issues]);

  return { statuses, issueStatusNameById };
}
