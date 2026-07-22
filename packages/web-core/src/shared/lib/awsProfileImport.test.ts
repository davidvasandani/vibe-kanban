import { describe, expect, it } from 'vitest';
import {
  buildAwsImportRequest,
  buildAwsImportRows,
  defaultAwsSessionName,
  isAwsImportBlocked,
  normalizeAwsProfilePart,
} from './awsProfileImport';

describe('AWS bulk import naming', () => {
  it('normalizes account and session names', () => {
    expect(normalizeAwsProfilePart('  My Account / Prod  ')).toBe(
      'My-Account-Prod'
    );
    expect(defaultAwsSessionName('https://my-org.awsapps.com/start')).toBe(
      'my-org'
    );
  });

  it('disambiguates duplicate generated names and classifies collisions', () => {
    const rows = buildAwsImportRows(
      [
        {
          account_id: '123456789012',
          account_name: 'Shared',
          roles: ['Admin'],
        },
        {
          account_id: '210987654321',
          account_name: 'Shared',
          roles: ['Admin'],
        },
      ],
      [
        {
          profile: {
            name: 'Shared.Admin',
            sso_start_url: 'https://x.awsapps.com/start',
            sso_region: 'us-east-1',
            sso_account_id: '123456789012',
            sso_role_name: 'Admin',
            region: null,
            output: null,
          },
          auth: { status: 'unauthenticated' },
          editable: true,
        },
      ]
    );
    expect(rows.map((row) => row.name)).toEqual([
      'Shared.Admin',
      'Shared.Admin.4321',
    ]);
    expect(rows[0].conflict).toBe('editable');
    expect(isAwsImportBlocked(rows)).toBe(true);
    rows[0].overwrite = true;
    expect(isAwsImportBlocked(rows)).toBe(false);
    rows[1].selected = false;
    expect(buildAwsImportRequest('work', ' us-west-2 ', 'json', rows)).toEqual({
      session_name: 'work',
      region: 'us-west-2',
      output: 'json',
      profiles: [
        {
          name: 'Shared.Admin',
          sso_account_id: '123456789012',
          sso_role_name: 'Admin',
          overwrite: true,
        },
      ],
    });
  });
});
