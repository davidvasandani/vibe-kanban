import { describe, expect, it } from 'vitest';
import { BaseCodingAgent } from 'shared/types';
import { agentsApi } from './api';

describe('agentsApi.getDiscoveredOptionsStreamUrl', () => {
  it('gives every caller for a session the same URL', () => {
    const fromModelSelector = agentsApi.getDiscoveredOptionsStreamUrl(
      BaseCodingAgent.CLAUDE_CODE,
      { workspaceId: 'ws-1', sessionId: 'session-1' }
    );
    const fromEditor = agentsApi.getDiscoveredOptionsStreamUrl(
      BaseCodingAgent.CLAUDE_CODE,
      { sessionId: 'session-1', repoId: 'repo-1' }
    );

    expect(fromModelSelector).toBe(fromEditor);
    expect(fromModelSelector).toBe(
      '/api/agents/discovered-options/ws?executor=CLAUDE_CODE&session_id=session-1'
    );
  });

  it('keeps repo scoping when there is no session', () => {
    expect(
      agentsApi.getDiscoveredOptionsStreamUrl(BaseCodingAgent.CLAUDE_CODE, {
        repoId: 'repo-1',
      })
    ).toBe(
      '/api/agents/discovered-options/ws?executor=CLAUDE_CODE&repo_id=repo-1'
    );
  });
});
