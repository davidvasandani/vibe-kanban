export const MCP_SERVER_IDENTIFIER_PATTERN = /^[a-zA-Z0-9_-]+$/;

export function isValidMcpServerIdentifier(value: string): boolean {
  return MCP_SERVER_IDENTIFIER_PATTERN.test(value);
}

export function suggestMcpServerIdentifier(value: string): string {
  let suggestion = '';
  let previousWasSeparator = false;
  for (const character of value.trim()) {
    if (/^[a-zA-Z0-9_-]$/.test(character)) {
      suggestion += character.toLowerCase();
      previousWasSeparator = false;
    } else if (!previousWasSeparator && suggestion.length > 0) {
      suggestion += '_';
      previousWasSeparator = true;
    }
  }
  suggestion = suggestion.replace(/_+$/, '');
  return suggestion || 'mcp_server';
}
