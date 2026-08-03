import { beforeEach, describe, expect, it, vi } from 'vitest';

// The store's `persist` middleware resolves `localStorage` at module-eval
// time, and web-core's vitest environment is `node`. Install a stub before
// the dynamic import below.
const backing = new Map<string, string>();

vi.stubGlobal('localStorage', {
  getItem: (key: string) => backing.get(key) ?? null,
  setItem: (key: string, value: string) => {
    backing.set(key, value);
  },
  removeItem: (key: string) => {
    backing.delete(key);
  },
  clear: () => backing.clear(),
  key: (i: number) => Array.from(backing.keys())[i] ?? null,
  get length() {
    return backing.size;
  },
});

async function loadStore() {
  vi.resetModules();
  return import('./useMetricsDrawerStore');
}

describe('useMetricsDrawerStore', () => {
  beforeEach(() => {
    backing.clear();
  });

  it('starts closed at the default width with nothing selected', async () => {
    const { useMetricsDrawerStore, METRICS_DRAWER_DEFAULT_WIDTH } =
      await loadStore();
    const state = useMetricsDrawerStore.getState();
    expect(state.open).toBe(false);
    expect(state.width).toBe(METRICS_DRAWER_DEFAULT_WIDTH);
    expect(state.selectedNodeId).toBeNull();
    expect(state.expandedPanels).toEqual({});
  });

  it('clamps the width to the 360..720 drag range', async () => {
    const {
      useMetricsDrawerStore,
      METRICS_DRAWER_MIN_WIDTH,
      METRICS_DRAWER_MAX_WIDTH,
    } = await loadStore();
    useMetricsDrawerStore.getState().setWidth(10);
    expect(useMetricsDrawerStore.getState().width).toBe(
      METRICS_DRAWER_MIN_WIDTH
    );
    useMetricsDrawerStore.getState().setWidth(5000);
    expect(useMetricsDrawerStore.getState().width).toBe(
      METRICS_DRAWER_MAX_WIDTH
    );
    useMetricsDrawerStore.getState().setWidth(500);
    expect(useMetricsDrawerStore.getState().width).toBe(500);
  });

  it('toggles panels independently', async () => {
    const { useMetricsDrawerStore } = await loadStore();
    useMetricsDrawerStore.getState().togglePanel('cpu');
    useMetricsDrawerStore.getState().setPanelExpanded('memory', true);
    useMetricsDrawerStore.getState().togglePanel('memory');
    expect(useMetricsDrawerStore.getState().expandedPanels).toEqual({
      cpu: true,
      memory: false,
    });
  });

  it('round-trips every preference through localStorage (FR-13)', async () => {
    const { useMetricsDrawerStore, METRICS_DRAWER_STORAGE_KEY } =
      await loadStore();

    useMetricsDrawerStore.getState().toggleOpen();
    useMetricsDrawerStore.getState().setWidth(512);
    useMetricsDrawerStore.getState().setSelectedNodeId('think3');
    useMetricsDrawerStore.getState().setPanelExpanded('disks', true);

    const raw = backing.get(METRICS_DRAWER_STORAGE_KEY);
    expect(raw).toBeDefined();
    expect(JSON.parse(raw as string).state).toEqual({
      open: true,
      width: 512,
      selectedNodeId: 'think3',
      expandedPanels: { disks: true },
    });

    // Re-evaluate the module: this is the page reload.
    const reloaded = await loadStore();
    expect(reloaded.useMetricsDrawerStore.getState()).toMatchObject({
      open: true,
      width: 512,
      selectedNodeId: 'think3',
      expandedPanels: { disks: true },
    });
  });

  it('repairs an out-of-range persisted width on rehydrate', async () => {
    const { METRICS_DRAWER_STORAGE_KEY, METRICS_DRAWER_MAX_WIDTH } =
      await loadStore();
    backing.set(
      METRICS_DRAWER_STORAGE_KEY,
      JSON.stringify({
        state: {
          open: true,
          width: 9000,
          selectedNodeId: null,
          expandedPanels: {},
        },
        version: 0,
      })
    );

    const reloaded = await loadStore();
    expect(reloaded.useMetricsDrawerStore.getState().width).toBe(
      METRICS_DRAWER_MAX_WIDTH
    );
    expect(reloaded.useMetricsDrawerStore.getState().open).toBe(true);
  });
});
