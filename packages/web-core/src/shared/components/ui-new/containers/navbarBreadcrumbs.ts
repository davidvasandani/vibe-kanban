import type { NavbarBreadcrumbItem } from '@vibe/ui/components/Navbar';

export const UNAVAILABLE_ISSUE_BREADCRUMB_LABEL = 'Issue unavailable';
export const UNAVAILABLE_PROJECT_BREADCRUMB_LABEL = 'Project unavailable';

export type WorkspaceBreadcrumbProjectState =
  | { kind: 'loading' }
  | { kind: 'resolved'; label: string; onClick: () => void }
  | { kind: 'unavailable' };

export type WorkspaceBreadcrumbIssueState =
  | { kind: 'none' }
  | { kind: 'loading' }
  | { kind: 'resolved'; label: string; onClick: () => void }
  | { kind: 'unavailable' };

interface BuildWorkspaceBreadcrumbsOptions {
  shouldResolve: boolean;
  projectState: WorkspaceBreadcrumbProjectState;
  workspaceLabel: string;
  issueState: WorkspaceBreadcrumbIssueState;
}

export function buildWorkspaceBreadcrumbs({
  shouldResolve,
  projectState,
  workspaceLabel,
  issueState,
}: BuildWorkspaceBreadcrumbsOptions): NavbarBreadcrumbItem[] | undefined {
  if (
    !shouldResolve ||
    projectState.kind === 'loading' ||
    issueState.kind === 'loading'
  ) {
    return undefined;
  }

  const items: NavbarBreadcrumbItem[] = [
    projectState.kind === 'resolved'
      ? { label: projectState.label, onClick: projectState.onClick }
      : { label: UNAVAILABLE_PROJECT_BREADCRUMB_LABEL },
  ];

  if (issueState.kind === 'resolved') {
    items.push({ label: issueState.label, onClick: issueState.onClick });
  } else if (issueState.kind === 'unavailable') {
    items.push({ label: UNAVAILABLE_ISSUE_BREADCRUMB_LABEL });
  }

  if (workspaceLabel) {
    items.push({ label: workspaceLabel });
  }

  return items.length > 1 ? items : undefined;
}
