import { describe, expect, it } from 'vitest';
import {
  isValidMcpServerIdentifier,
  suggestMcpServerIdentifier,
} from './mcpServerIdentifier';

describe('MCP server identifiers', () => {
  it.each([
    ['Atlassian Rovo', 'atlassian_rovo'],
    ['  Rovo...Cloud!  ', 'rovo_cloud'],
    ['Vibe-Kanban', 'vibe-kanban'],
    ['工具', 'mcp_server'],
  ])('suggests a protocol-safe identifier for %s', (input, expected) => {
    expect(suggestMcpServerIdentifier(input)).toBe(expected);
  });

  it('accepts only protocol-safe identifiers', () => {
    expect(isValidMcpServerIdentifier('atlassian_rovo')).toBe(true);
    expect(isValidMcpServerIdentifier('Atlassian Rovo')).toBe(false);
  });
});
