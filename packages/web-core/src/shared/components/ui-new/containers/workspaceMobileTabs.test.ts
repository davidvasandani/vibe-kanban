import { describe, expect, it } from 'vitest';
import {
  getAvailableWorkspaceMobileTabs,
  getWorkspaceMobileTabFallback,
} from './workspaceMobileTabs';

describe('workspace mobile tab availability', () => {
  it('exposes the existing right drawer destination for a selected workspace', () => {
    expect(
      getAvailableWorkspaceMobileTabs({
        hasWorkspaceRoute: true,
        isCreateMode: false,
      }).map((tab) => tab.id)
    ).toContain('git');
  });

  it.each([
    { hasWorkspaceRoute: false, isCreateMode: false },
    { hasWorkspaceRoute: false, isCreateMode: true },
    { hasWorkspaceRoute: true, isCreateMode: true },
  ])('omits the drawer without usable workspace content: %o', (state) => {
    expect(
      getAvailableWorkspaceMobileTabs(state).map((tab) => tab.id)
    ).not.toContain('git');
  });

  it('recovers an unavailable drawer selection to usable content', () => {
    expect(
      getWorkspaceMobileTabFallback('git', {
        hasWorkspaceRoute: false,
        isCreateMode: false,
      })
    ).toBe('workspaces');
    expect(
      getWorkspaceMobileTabFallback('git', {
        hasWorkspaceRoute: false,
        isCreateMode: true,
      })
    ).toBe('chat');
  });

  it('preserves available and unrelated active tabs', () => {
    expect(
      getWorkspaceMobileTabFallback('git', {
        hasWorkspaceRoute: true,
        isCreateMode: false,
      })
    ).toBe('git');
    expect(
      getWorkspaceMobileTabFallback('changes', {
        hasWorkspaceRoute: false,
        isCreateMode: false,
      })
    ).toBe('changes');
  });
});
