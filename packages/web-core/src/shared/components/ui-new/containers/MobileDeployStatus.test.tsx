import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import {
  MobileDeployStatus,
  formatDeploymentAge,
} from '@vibe/ui/components/MobileDeployStatus';

describe('formatDeploymentAge', () => {
  const startedAt = '2026-08-09T12:00:00Z';
  const at = (elapsedMs: number) =>
    formatDeploymentAge(startedAt, Date.parse(startedAt) + elapsedMs);

  it('uses compact completed units at each boundary', () => {
    expect(at(59_999)).toBe('now');
    expect(at(60_000)).toBe('1m');
    expect(at(59 * 60_000)).toBe('59m');
    expect(at(60 * 60_000)).toBe('1h');
    expect(at(23 * 60 * 60_000)).toBe('23h');
    expect(at(24 * 60 * 60_000)).toBe('1d');
  });

  it('fails soft for missing or invalid timestamps', () => {
    expect(formatDeploymentAge(null)).toBeNull();
    expect(formatDeploymentAge('not-a-date')).toBeNull();
  });
});

describe('MobileDeployStatus', () => {
  it('renders a production revision as a commit link with its age', () => {
    const html = renderToStaticMarkup(
      <MobileDeployStatus
        revision="ac5bedd"
        startedAt={new Date(Date.now() - 2 * 60 * 60_000).toISOString()}
      />
    );

    expect(html).toContain('/commit/ac5bedd');
    expect(html).toContain('ac5bedd');
    expect(html).toContain('2h');
  });

  it('renders dev as text and omits unusable metadata safely', () => {
    const devHtml = renderToStaticMarkup(
      <MobileDeployStatus revision="dev" startedAt={null} />
    );
    expect(devHtml).toContain('dev');
    expect(devHtml).not.toContain('<a');
    expect(
      renderToStaticMarkup(
        <MobileDeployStatus revision={null} startedAt="invalid" />
      )
    ).toBe('');
  });
});
