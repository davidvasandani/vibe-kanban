import type { BaseCodingAgent } from 'shared/types';
import type {
  CreateIssueRequest,
  Issue,
  ProjectStatus,
} from 'shared/remote-types';
import { toPrettyCase } from './string';

export type McpDebugIssueIdentity = {
  serverName: string;
  executor: BaseCodingAgent;
};

export type McpDebugIssueInput = McpDebugIssueIdentity & {
  diagnostic: string;
};

export type McpDebugAvailability =
  | { available: true; status: ProjectStatus }
  | { available: false; reason: 'no-project' | 'no-status' };

const activeMcpDebugCreations = new Set<string>();

export function mcpDiagnosticText(
  error: string | null | undefined,
  fallback: string
): string {
  return typeof error === 'string' && error.length > 0 ? error : fallback;
}

export function markdownFenceFor(text: string): string {
  let longestBacktickRun = 0;
  for (const match of text.matchAll(/`+/g)) {
    longestBacktickRun = Math.max(longestBacktickRun, match[0].length);
  }
  return '`'.repeat(Math.max(3, longestBacktickRun + 1));
}

export function buildMcpDebugIssueTitle({
  serverName,
  executor,
}: McpDebugIssueIdentity): string {
  return `Debug MCP failure: ${serverName} on ${toPrettyCase(executor)}`;
}

export function buildMcpDebugIssueDescription({
  serverName,
  executor,
  diagnostic,
}: McpDebugIssueInput): string {
  const fence = markdownFenceFor(diagnostic);
  return [
    'Investigate this saved MCP server connectivity failure.',
    '',
    `MCP server: ${serverName}`,
    `Executor: ${toPrettyCase(executor)}`,
    '',
    'Diagnostic:',
    `${fence}text`,
    diagnostic,
    fence,
    '',
    'Instructions:',
    '- Reproduce the MCP assignment test failure.',
    '- Identify the root cause without changing secret-redaction or OAuth semantics.',
    '- Implement the smallest fix that preserves existing MCP assignment behavior.',
    '- Run relevant tests and report the root cause, fix, and verification.',
  ].join('\n');
}

export function firstProjectStatusBySortOrder(
  statuses: ProjectStatus[]
): ProjectStatus | null {
  return [...statuses].sort((a, b) => a.sort_order - b.sort_order)[0] ?? null;
}

export function mcpDebugAvailability(
  hasProjectContext: boolean,
  statuses: ProjectStatus[]
): McpDebugAvailability {
  if (!hasProjectContext) return { available: false, reason: 'no-project' };
  const status = firstProjectStatusBySortOrder(statuses);
  return status
    ? { available: true, status }
    : { available: false, reason: 'no-status' };
}

export function topOfStatusSortOrder(
  issues: Issue[],
  statusId: string
): number {
  const statusIssues = issues.filter((issue) => issue.status_id === statusId);
  const minSortOrder =
    statusIssues.length > 0
      ? Math.min(...statusIssues.map((issue) => issue.sort_order))
      : 0;
  return minSortOrder - 1;
}

export function buildMcpDebugIssueRequest({
  projectId,
  status,
  issues,
  serverName,
  executor,
  diagnostic,
}: McpDebugIssueInput & {
  projectId: string;
  status: ProjectStatus;
  issues: Issue[];
}): CreateIssueRequest {
  return {
    project_id: projectId,
    status_id: status.id,
    title: buildMcpDebugIssueTitle({ serverName, executor }),
    description: buildMcpDebugIssueDescription({
      serverName,
      executor,
      diagnostic,
    }),
    priority: null,
    sort_order: topOfStatusSortOrder(issues, status.id),
    start_date: null,
    target_date: null,
    completed_at: null,
    parent_issue_id: null,
    parent_issue_sort_order: null,
    extension_metadata: null,
  };
}

export function canStartMcpDebugCreation(status: string | undefined): boolean {
  return status !== 'creating';
}

export function resettableMcpDebugKeys(
  resultKeys: Iterable<string>,
  creatingKeys: ReadonlySet<string>
): string[] {
  return [...resultKeys].filter((key) => !creatingKeys.has(key));
}

export function mcpDebugCreationKey(
  projectId: string,
  assignmentKey: string
): string {
  return JSON.stringify([projectId, assignmentKey]);
}

export function acquireMcpDebugCreation(key: string): boolean {
  if (activeMcpDebugCreations.has(key)) return false;
  activeMcpDebugCreations.add(key);
  return true;
}

export function releaseMcpDebugCreation(key: string): void {
  activeMcpDebugCreations.delete(key);
}
