import { describe, expect, it, vi } from 'vitest';
import {
  buildWorkspaceBreadcrumbs,
  UNAVAILABLE_ISSUE_BREADCRUMB_LABEL,
} from './navbarBreadcrumbs';

describe('buildWorkspaceBreadcrumbs', () => {
  it('builds resolved linked issue breadcrumbs with issue navigation', () => {
    const linkedProjectId = 'project-id-1';
    const linkedIssueId = 'issue-id-1';
    const onProjectClick = vi.fn();
    const goToProjectIssue = vi.fn();

    const breadcrumbs = buildWorkspaceBreadcrumbs({
      shouldResolve: true,
      project: { name: 'Project Alpha' },
      workspaceLabel: 'Workspace One',
      issueState: {
        kind: 'resolved',
        label: 'ALPHA-123',
        onClick: () => goToProjectIssue(linkedProjectId, linkedIssueId),
      },
      onProjectClick,
    });

    expect(breadcrumbs?.map((item) => item.label)).toEqual([
      'Project Alpha',
      'ALPHA-123',
      'Workspace One',
    ]);
    expect(
      breadcrumbs?.filter((item) => item.label === 'ALPHA-123')
    ).toHaveLength(1);

    breadcrumbs?.[1]?.onClick?.();
    expect(goToProjectIssue).toHaveBeenCalledTimes(1);
    expect(goToProjectIssue).toHaveBeenCalledWith(
      linkedProjectId,
      linkedIssueId
    );
    expect(onProjectClick).not.toHaveBeenCalled();
  });

  it('defers linked issue breadcrumbs while the issue label is loading', () => {
    const breadcrumbs = buildWorkspaceBreadcrumbs({
      shouldResolve: true,
      project: { name: 'Project Alpha' },
      workspaceLabel: 'Workspace One',
      issueState: { kind: 'loading' },
      onProjectClick: vi.fn(),
    });

    expect(breadcrumbs).toBeUndefined();
    expect(breadcrumbs?.map((item) => item.label)).not.toEqual([
      'Project Alpha',
      'Workspace One',
    ]);
    expect(
      breadcrumbs?.some((item) => item.label === 'issue-uuid-123')
    ).not.toBe(true);
  });

  it('builds unavailable linked issue breadcrumbs without issue navigation', () => {
    const breadcrumbs = buildWorkspaceBreadcrumbs({
      shouldResolve: true,
      project: { name: 'Project Alpha' },
      workspaceLabel: 'Workspace One',
      issueState: { kind: 'unavailable' },
      onProjectClick: vi.fn(),
    });

    expect(breadcrumbs?.map((item) => item.label)).toEqual([
      'Project Alpha',
      UNAVAILABLE_ISSUE_BREADCRUMB_LABEL,
      'Workspace One',
    ]);
    expect(breadcrumbs?.[1]).toMatchObject({
      label: UNAVAILABLE_ISSUE_BREADCRUMB_LABEL,
    });
    expect(breadcrumbs?.[1]?.onClick).toBeUndefined();
    expect(breadcrumbs?.some((item) => item.label === 'issue-uuid-123')).toBe(
      false
    );
    expect(breadcrumbs?.map((item) => item.label)).not.toEqual([
      'Project Alpha',
      'Workspace One',
    ]);
  });

  it('preserves unlinked workspace breadcrumbs', () => {
    const breadcrumbs = buildWorkspaceBreadcrumbs({
      shouldResolve: true,
      project: { name: 'Project Alpha' },
      workspaceLabel: 'Workspace One',
      issueState: { kind: 'none' },
      onProjectClick: vi.fn(),
    });

    expect(breadcrumbs?.map((item) => item.label)).toEqual([
      'Project Alpha',
      'Workspace One',
    ]);
    expect(breadcrumbs).toHaveLength(2);
  });

  it('does not build workspace breadcrumbs when resolution is inapplicable', () => {
    const breadcrumbs = buildWorkspaceBreadcrumbs({
      shouldResolve: false,
      project: { name: 'Project Alpha' },
      workspaceLabel: 'Workspace One',
      issueState: {
        kind: 'resolved',
        label: 'ALPHA-123',
        onClick: vi.fn(),
      },
      onProjectClick: vi.fn(),
    });

    expect(breadcrumbs).toBeUndefined();
  });
});
