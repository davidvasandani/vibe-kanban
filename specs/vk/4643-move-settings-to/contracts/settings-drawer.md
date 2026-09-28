# Contract: Settings drawer interface

```ts
// Interface contract for the Settings drawer (web-core). Illustrative only; the
// implementation lives in packages/web-core/src/shared/stores/useSettingsDrawerStore.ts
// and packages/web-core/src/shared/dialogs/settings/SettingsDialog.tsx.

import type { SettingsDialogProps } from '@/shared/dialogs/settings/SettingsDialog';

export interface SettingsDrawerState {
  isOpen: boolean;
  width: number;
  requestClose: (() => void | Promise<void>) | null;
  setOpen(open: boolean): void;
  setWidth(width: number): void; // persists
  registerCloseRequest(fn: (() => void | Promise<void>) | null): void;
}

export declare function clampSettingsDrawerWidth(
  width: number,
  viewportWidth: number
): number;

/** Pixels the app shell must reserve on the right (0 when closed or mobile). */
export declare function useSettingsDrawerInset(isMobile: boolean): number;

/** Unchanged: opens (or retargets, if already open) and resolves on close. */
export declare const SettingsDialog: {
  show(props?: SettingsDialogProps): Promise<void>;
};

/** Opens when closed; routes through the unsaved-changes guard when open. */
export declare function toggleSettingsDrawer(
  props?: SettingsDialogProps
): Promise<void>;
```
