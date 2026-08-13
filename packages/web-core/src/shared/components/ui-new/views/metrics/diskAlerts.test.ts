import { describe, expect, it } from 'vitest';
import type { DiskAlertThresholds, FilesystemSample } from 'shared/types';
import { classifyFilesystem } from './diskAlerts';

const thresholds: DiskAlertThresholds = {
  warning_free_percent: 10,
  warning_free_bytes: 5 * 1024 ** 3,
  critical_free_percent: 2,
  critical_free_bytes: 1024 ** 3,
};

const filesystem = (available: number, total = 100 * 1024 ** 3) =>
  ({
    mount_point: '/',
    device: '/dev/root',
    fs_type: 'ext4',
    total_bytes: total,
    used_bytes: total - available,
    available_bytes: available,
  }) as FilesystemSample;

describe('classifyFilesystem', () => {
  it('uses the more conservative percent or byte rule', () => {
    expect(classifyFilesystem(filesystem(6 * 1024 ** 3), thresholds)?.severity).toBe(
      'warning'
    );
    expect(
      classifyFilesystem(filesystem(4 * 1024 ** 3, 20 * 1024 ** 3), thresholds)
        ?.severity
    ).toBe('warning');
  });

  it('gives critical precedence', () => {
    expect(classifyFilesystem(filesystem(512 * 1024 ** 2), thresholds)?.severity).toBe(
      'critical'
    );
  });

  it('does not alert at exact boundaries or for absent facts', () => {
    expect(
      classifyFilesystem(filesystem(5 * 1024 ** 3, 50 * 1024 ** 3), thresholds)
    ).toBeNull();
    expect(
      classifyFilesystem(
        { ...filesystem(1), available_bytes: null },
        thresholds
      )
    ).toBeNull();
  });
});
