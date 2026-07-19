import { describe, expect, it } from 'vitest';
import { BaseCodingAgent } from 'shared/types';
import type { Issue, ProjectStatus } from 'shared/remote-types';
import {
  acquireMcpDebugCreation,
  buildMcpDebugIssueDescription,
  buildMcpDebugIssueRequest,
  buildMcpDebugIssueTitle,
  canStartMcpDebugCreation,
  markdownFenceFor,
  mcpDebugAvailability,
  mcpDebugCreationKey,
  mcpDiagnosticText,
  resettableMcpDebugKeys,
  releaseMcpDebugCreation,
  topOfStatusSortOrder,
} from './mcpDebugIssue';

const backlog: ProjectStatus = {
  id: 'status-backlog',
  project_id: 'project-1',
  name: 'Backlog',
  color: '#fff',
  sort_order: 10,
  hidden: false,
  created_at: '2026-01-01T00:00:00Z',
};

const todo: ProjectStatus = {
  ...backlog,
  id: 'status-todo',
  name: 'Todo',
  sort_order: 1,
};

const issue: Issue = {
  id: 'issue-1',
  project_id: 'project-1',
  issue_number: 1,
  simple_id: 'PROJ-1',
  status_id: 'status-todo',
  title: 'Existing',
  description: null,
  priority: null,
  start_date: null,
  target_date: null,
  completed_at: null,
  sort_order: 4,
  parent_issue_id: null,
  parent_issue_sort_order: null,
  extension_metadata: null,
  creator_user_id: null,
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
};

describe('mcpDebugIssue', () => {
  it('preserves exact multiline diagnostics', () => {
    const diagnostic = 'spawn failed\nstderr line 1\nstderr line 2';

    expect(mcpDiagnosticText(diagnostic, 'fallback')).toBe(diagnostic);
    expect(
      buildMcpDebugIssueDescription({
        serverName: 'linear',
        executor: BaseCodingAgent.CODEX,
        diagnostic,
      })
    ).toContain(`\n${diagnostic}\n`);
  });

  it('uses fallback diagnostics when the backend diagnostic is missing', () => {
    expect(mcpDiagnosticText(undefined, 'No diagnostic returned.')).toBe(
      'No diagnostic returned.'
    );
    expect(mcpDiagnosticText('', 'No diagnostic returned.')).toBe(
      'No diagnostic returned.'
    );
  });

  it('builds a deterministic title with server and executor identity', () => {
    expect(
      buildMcpDebugIssueTitle({
        serverName: 'filesystem',
        executor: BaseCodingAgent.CLAUDE_CODE,
      })
    ).toBe('Debug MCP failure: filesystem on Claude Code');
  });

  it('uses a Markdown fence longer than any diagnostic fence marker', () => {
    const diagnostic = 'before\n```text\nnested\n````\nafter';
    const fence = markdownFenceFor(diagnostic);
    const description = buildMcpDebugIssueDescription({
      serverName: 'tools',
      executor: BaseCodingAgent.GEMINI,
      diagnostic,
    });

    expect(fence).toBe('`````');
    expect(description).toContain(`Diagnostic:\n${fence}text\n${diagnostic}`);
    expect(description).toContain(`\n${fence}\n\nInstructions:`);
  });

  it('reports Debug unavailable outside a project context', () => {
    expect(mcpDebugAvailability(false, [todo])).toEqual({
      available: false,
      reason: 'no-project',
    });
  });

  it('reports Debug unavailable when no project status exists', () => {
    expect(mcpDebugAvailability(true, [])).toEqual({
      available: false,
      reason: 'no-status',
    });
  });

  it('builds the successful Debug issue payload at the top status column', () => {
    const availability = mcpDebugAvailability(true, [backlog, todo]);
    expect(availability.available).toBe(true);
    if (!availability.available) throw new Error('expected status');

    const request = buildMcpDebugIssueRequest({
      projectId: 'project-1',
      status: availability.status,
      issues: [{ ...issue, sort_order: -2 }],
      serverName: 'github',
      executor: BaseCodingAgent.CODEX,
      diagnostic: 'transport error',
    });

    expect(request).toMatchObject({
      project_id: 'project-1',
      status_id: 'status-todo',
      title: 'Debug MCP failure: github on Codex',
      priority: null,
      sort_order: -3,
      start_date: null,
      target_date: null,
      completed_at: null,
      parent_issue_id: null,
      parent_issue_sort_order: null,
      extension_metadata: null,
    });
    expect(request.description).toContain('transport error');
    expect(request.description).toContain(
      'without changing secret-redaction or OAuth semantics'
    );
  });

  it('computes top-of-column sort order for empty columns', () => {
    expect(topOfStatusSortOrder([], 'status-todo')).toBe(-1);
  });

  it('prevents duplicate Debug creation while creating', () => {
    expect(canStartMcpDebugCreation('creating')).toBe(false);
    expect(canStartMcpDebugCreation('idle')).toBe(true);
    expect(canStartMcpDebugCreation('error')).toBe(true);
    expect(canStartMcpDebugCreation('success')).toBe(true);
  });

  it('does not reset an in-flight Debug guard when an assignment is retested', () => {
    expect(
      resettableMcpDebugKeys(
        ['server:codex', 'server:claude'],
        new Set(['server:codex'])
      )
    ).toEqual(['server:claude']);
  });

  it('guards duplicate creation across component instances until persistence settles', () => {
    const key = mcpDebugCreationKey('project-1', 'server:codex');
    expect(acquireMcpDebugCreation(key)).toBe(true);
    expect(acquireMcpDebugCreation(key)).toBe(false);
    releaseMcpDebugCreation(key);
    expect(acquireMcpDebugCreation(key)).toBe(true);
    releaseMcpDebugCreation(key);
  });
});
