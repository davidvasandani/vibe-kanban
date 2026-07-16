import type { NavbarBreadcrumbItem } from '@vibe/ui/components/Navbar';

export const UNAVAILABLE_ISSUE_BREADCRUMB_LABEL = 'Issue unavailable';

export type WorkspaceBreadcrumbIssueState =
  | { kind: 'none' }
  | { kind: 'loading' }
  | { kind: 'resolved'; label: string; onClick: () => void }
  | { kind: 'unavailable' };

interface BuildWorkspaceBreadcrumbsOptions {
  shouldResolve: boolean;
  project: { name: string } | null | undefined;
  workspaceLabel: string;
  issueState: WorkspaceBreadcrumbIssueState;
  onProjectClick: () => void;
}

export function buildWorkspaceBreadcrumbs({
  shouldResolve,
  project,
  workspaceLabel,
  issueState,
  onProjectClick,
}: BuildWorkspaceBreadcrumbsOptions): NavbarBreadcrumbItem[] | undefined {
  if (!shouldResolve || !project || issueState.kind === 'loading') {
    return undefined;
  }

  const items: NavbarBreadcrumbItem[] = [
    { label: project.name, onClick: onProjectClick },
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
