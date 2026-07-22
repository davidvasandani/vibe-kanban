import type {
  AwsProfileImportCandidate,
  AwsProfileImportRequest,
  AwsSsoAccount,
  AwsSsoProfileStatus,
} from 'shared/types';

const INVALID_NAME_CHARS = /[^A-Za-z0-9_.@-]+/g;

export function normalizeAwsProfilePart(value: string): string {
  return value
    .trim()
    .replace(INVALID_NAME_CHARS, '-')
    .replace(/-+/g, '-')
    .replace(/^[.-]+|[.-]+$/g, '');
}

export function defaultAwsSessionName(startUrl: string): string {
  try {
    const first = new URL(startUrl).hostname.split('.')[0];
    return normalizeAwsProfilePart(first) || 'aws-sso';
  } catch {
    return 'aws-sso';
  }
}

export type AwsImportRow = AwsProfileImportCandidate & {
  key: string;
  account_name: string;
  selected: boolean;
  conflict: 'none' | 'editable' | 'protected';
};

export function buildAwsImportRows(
  accounts: AwsSsoAccount[],
  existing: AwsSsoProfileStatus[]
): AwsImportRow[] {
  const existingByName = new Map(
    existing.map((status) => [status.profile.name, status.editable])
  );
  const used = new Set<string>();
  return accounts.flatMap((account) =>
    account.roles.map((role) => {
      const base =
        `${normalizeAwsProfilePart(account.account_name)}.${normalizeAwsProfilePart(role)}`.replace(
          /^\.|\.$/g,
          ''
        ) || `aws-${account.account_id}`;
      let name = base;
      if (used.has(name)) name = `${base}.${account.account_id.slice(-4)}`;
      let suffix = 2;
      while (used.has(name))
        name = `${base}.${account.account_id.slice(-4)}-${suffix++}`;
      used.add(name);
      const editable = existingByName.get(name);
      return {
        key: `${account.account_id}:${role}`,
        account_name: account.account_name,
        name,
        sso_account_id: account.account_id,
        sso_role_name: role,
        overwrite: false,
        selected: true,
        conflict:
          editable === undefined ? 'none' : editable ? 'editable' : 'protected',
      };
    })
  );
}

export function isAwsImportBlocked(rows: AwsImportRow[]): boolean {
  const selected = rows.filter((row) => row.selected);
  return (
    selected.length === 0 ||
    selected.some(
      (row) =>
        row.conflict === 'protected' ||
        (row.conflict === 'editable' && !row.overwrite) ||
        !/^[A-Za-z0-9_.@-]{1,128}$/.test(row.name) ||
        row.name === 'default'
    )
  );
}

export function buildAwsImportRequest(
  sessionName: string,
  region: string,
  output: string,
  rows: AwsImportRow[]
): AwsProfileImportRequest {
  return {
    session_name: sessionName,
    region: region.trim(),
    output: output || null,
    profiles: rows
      .filter((row) => row.selected)
      .map(({ name, sso_account_id, sso_role_name, overwrite }) => ({
        name,
        sso_account_id,
        sso_role_name,
        overwrite,
      })),
  };
}
