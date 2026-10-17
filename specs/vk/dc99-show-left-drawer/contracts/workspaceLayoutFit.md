# Contract — `shared/lib/workspaceLayoutFit.ts`

```ts
export const LEFT_SIDEBAR_WIDTH = 300;
export const RIGHT_SIDEBAR_WIDTH = 300;
export const MIN_CHAT_WIDTH = 480;
export const MIN_RIGHT_MAIN_PANEL_WIDTH = 400;

export interface LeftSidebarFitInput {
  /** Measured width of the workspace layout root, in CSS px. */
  width: number;
  isChatShown: boolean;
  isRightMainPanelOpen: boolean;
  isRightSidebarShown: boolean;
}

/** Width the layout needs to show the left drawer beside the open content. */
export function requiredWidthForLeftSidebar(input: Omit<LeftSidebarFitInput, 'width'>): number;

/** True when `width` ≥ `requiredWidthForLeftSidebar(input)`. */
export function leftSidebarFits(input: LeftSidebarFitInput): boolean;
```

The store contract is `selectIsLeftSidebarShown(state): boolean`, together
with `setLeftSidebarHasSpace(value: boolean): void` and the `toggleLeftSidebar`
transitions in `data-model.md`.
