import { describe, expect, it } from 'vitest';
import {
  applyPreviewRoute,
  getPreviewRoute,
  stripPreviewTransportParams,
} from './previewUrlState';

describe('preview URL state', () => {
  it('extracts path, query, and fragment while removing transport metadata', () => {
    expect(
      getPreviewRoute(
        'http://3000.localhost:4000/worksheets/7?mode=timed&_refresh=2&_vk_workspace=ws#problem-3'
      )
    ).toBe('/worksheets/7?mode=timed#problem-3');
  });

  it('rebases a retained route onto the newly detected server origin', () => {
    expect(
      applyPreviewRoute(
        'http://localhost:5174/?_refresh=4',
        '/worksheets/7?mode=timed#problem-3'
      )
    ).toBe('http://localhost:5174/worksheets/7?mode=timed#problem-3');
  });

  it('rejects routes that could replace the detected origin', () => {
    expect(
      applyPreviewRoute('http://localhost:5174/', '//example.com/worksheet/7')
    ).toBeNull();
    expect(
      applyPreviewRoute('http://localhost:5174/', 'https://example.com/')
    ).toBeNull();
  });

  it('strips every preview transport parameter from user-visible URLs', () => {
    expect(
      stripPreviewTransportParams(
        'http://localhost:5174/worksheets/7?_refresh=1&_vk_workspace=ws&_vk_execution=ex&_vk_generation=2&mode=timed#problem-3'
      )
    ).toBe('http://localhost:5174/worksheets/7?mode=timed#problem-3');
  });

  it('returns null for malformed URLs', () => {
    expect(getPreviewRoute('not a URL')).toBeNull();
    expect(stripPreviewTransportParams('not a URL')).toBeNull();
    expect(applyPreviewRoute('not a URL', '/worksheet/7')).toBeNull();
  });
});
