import { describe, expect, it } from 'vitest';
import type {
  DiskAlertThresholds,
  FilesystemSample,
  MetricsNode,
} from 'shared/types';
import { classifyFilesystem, classifyNode } from './diskAlerts';

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
  it.each(['/boot', '/boot/', '/boot/efi'])(
    'ignores boot filesystem mounted at %s',
    (mountPoint) => {
      expect(
        classifyFilesystem(
          {
            ...filesystem(512 * 1024 ** 2),
            mount_point: mountPoint,
          },
          thresholds
        )
      ).toBeNull();
    }
  );

  it('keeps similarly named and constrained non-boot mounts eligible', () => {
    expect(
      classifyFilesystem(
        {
          ...filesystem(512 * 1024 ** 2),
          mount_point: '/bootstrap',
        },
        thresholds
      )?.severity
    ).toBe('critical');
  });

  it('uses the more conservative percent or byte rule', () => {
    expect(
      classifyFilesystem(filesystem(6 * 1024 ** 3), thresholds)?.severity
    ).toBe('warning');
    expect(
      classifyFilesystem(filesystem(4 * 1024 ** 3, 20 * 1024 ** 3), thresholds)
        ?.severity
    ).toBe('warning');
  });

  it('gives critical precedence', () => {
    expect(
      classifyFilesystem(filesystem(512 * 1024 ** 2), thresholds)?.severity
    ).toBe('critical');
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

  it('rejects internally inconsistent capacity facts', () => {
    expect(
      classifyFilesystem(
        {
          ...filesystem(1, 1024 ** 3),
          available_bytes: 2n * 1024n ** 3n,
        },
        thresholds
      )
    ).toBeNull();
    expect(
      classifyFilesystem(
        { ...filesystem(1), used_bytes: 101n * 1024n ** 3n },
        thresholds
      )
    ).toBeNull();
  });
});

describe('classifyNode availability', () => {
  const node = (status: MetricsNode['availability']): MetricsNode =>
    ({
      node_id: '00000000-0000-0000-0000-000000000001',
      availability: status,
      latest: { filesystems: [filesystem(512 * 1024 ** 2)] },
    }) as MetricsNode;

  it('does not alert from retained readings for unavailable nodes', () => {
    expect(
      classifyNode(
        node({ status: 'unreachable', reason: 'timeout' }),
        thresholds
      )
    ).toBeNull();
    expect(
      classifyNode(node({ status: 'not_implemented' }), thresholds)
    ).toBeNull();
  });

  it('retains stale evidence as a warning rather than a critical alert', () => {
    expect(
      classifyNode(
        node({ status: 'stale', since: '2026-08-13T10:00:00Z' }),
        thresholds
      )?.severity
    ).toBe('warning');
  });

  it('omits a node whose only constrained filesystem is a boot mount', () => {
    expect(
      classifyNode(
        {
          ...node({ status: 'available' }),
          latest: {
            filesystems: [
              {
                ...filesystem(512 * 1024 ** 2),
                mount_point: '/boot',
              },
            ],
          },
        } as MetricsNode,
        thresholds
      )
    ).toBeNull();
  });
});
