import { useCallback, useMemo, useRef } from 'react';
import { useScratch } from '@/shared/hooks/useScratch';
import { useDebouncedCallback } from '@/shared/hooks/useDebouncedCallback';
import {
  ScratchType,
  type PreviewSettingsData,
  type ScratchPayload,
} from 'shared/types';

export type ScreenSize = 'desktop' | 'mobile' | 'responsive';

export interface ResponsiveDimensions {
  width: number;
  height: number;
}

interface UsePreviewSettingsResult {
  // URL override
  overrideUrl: string | null;
  hasOverride: boolean;
  setOverrideUrl: (url: string) => void;
  clearOverride: () => Promise<void>;

  // Latest route within an auto-detected application
  currentRoute: string | null;
  setCurrentRoute: (route: string) => void;

  // Screen size
  screenSize: ScreenSize;
  responsiveDimensions: ResponsiveDimensions;
  setScreenSize: (size: ScreenSize) => void;
  setResponsiveDimensions: (dimensions: ResponsiveDimensions) => void;

  isLoading: boolean;
}

const DEFAULT_RESPONSIVE_DIMENSIONS: ResponsiveDimensions = {
  width: 800,
  height: 600,
};

/**
 * Hook to manage per-workspace preview settings (URL override and screen size).
 * Uses the scratch system for persistence.
 */
export function usePreviewSettings(
  workspaceId: string | undefined
): UsePreviewSettingsResult {
  const enabled = !!workspaceId;

  const {
    scratch,
    updateScratch,
    isLoading: isScratchLoading,
  } = useScratch(ScratchType.PREVIEW_SETTINGS, workspaceId ?? '', {
    enabled,
  });

  // Extract settings from scratch data
  const payload = scratch?.payload as ScratchPayload | undefined;
  const scratchData: PreviewSettingsData | undefined =
    payload?.type === 'PREVIEW_SETTINGS' ? payload.data : undefined;

  const overrideUrl = scratchData?.url ?? null;
  const hasOverride = overrideUrl !== null && overrideUrl.trim() !== '';
  const currentRoute = scratchData?.current_route ?? null;

  const screenSize: ScreenSize =
    (scratchData?.screen_size as ScreenSize) ?? 'desktop';
  const responsiveDimensions: ResponsiveDimensions = useMemo(
    () => ({
      width:
        scratchData?.responsive_width ?? DEFAULT_RESPONSIVE_DIMENSIONS.width,
      height:
        scratchData?.responsive_height ?? DEFAULT_RESPONSIVE_DIMENSIONS.height,
    }),
    [scratchData?.responsive_width, scratchData?.responsive_height]
  );

  const scratchVersion = scratch?.updated_at ?? null;
  const completeSettings: PreviewSettingsData = {
    url: overrideUrl ?? '',
    current_route: currentRoute,
    screen_size: screenSize,
    responsive_width: responsiveDimensions.width,
    responsive_height: responsiveDimensions.height,
  };
  const writeStateRef = useRef<{
    workspaceId: string | undefined;
    scratchVersion: string | null;
    settings: PreviewSettingsData;
    pending: number;
    chain: Promise<void>;
    ownScratchVersions: Set<string>;
  }>({
    workspaceId,
    scratchVersion,
    settings: completeSettings,
    pending: 0,
    chain: Promise.resolve(),
    ownScratchVersions: new Set(),
  });
  if (writeStateRef.current.workspaceId !== workspaceId) {
    writeStateRef.current = {
      workspaceId,
      scratchVersion,
      settings: completeSettings,
      pending: 0,
      chain: Promise.resolve(),
      ownScratchVersions: new Set(),
    };
  } else if (
    scratchVersion &&
    writeStateRef.current.ownScratchVersions.delete(scratchVersion)
  ) {
    writeStateRef.current.scratchVersion = scratchVersion;
  } else if (
    writeStateRef.current.pending === 0 &&
    writeStateRef.current.scratchVersion !== scratchVersion
  ) {
    writeStateRef.current.scratchVersion = scratchVersion;
    writeStateRef.current.settings = completeSettings;
  }

  // Helper to save settings
  const saveSettings = useCallback(
    async (updates: Partial<PreviewSettingsData>) => {
      if (!workspaceId) return;

      const writeState = writeStateRef.current;
      if (writeState.workspaceId !== workspaceId) return;

      writeState.settings = { ...writeState.settings, ...updates };
      const settings = { ...writeState.settings };
      writeState.pending += 1;
      writeState.chain = writeState.chain
        .then(async () => {
          const updatedScratch = await updateScratch({
            payload: {
              type: 'PREVIEW_SETTINGS',
              data: settings,
            },
          });
          writeState.ownScratchVersions.add(updatedScratch.updated_at);
        })
        .catch((e) => {
          console.error('[usePreviewSettings] Failed to save:', e);
        })
        .finally(() => {
          writeState.pending -= 1;
        });

      await writeState.chain;
    },
    [workspaceId, updateScratch]
  );

  // Debounced save for URL changes (frequent typing)
  const { debounced: debouncedSaveUrl, cancel: cancelSaveUrl } =
    useDebouncedCallback(async (url: string) => {
      await saveSettings({ url });
    }, 300);

  // Debounced save for responsive dimensions (frequent dragging)
  const { debounced: debouncedSaveDimensions } = useDebouncedCallback(
    async (dimensions: ResponsiveDimensions) => {
      await saveSettings({
        responsive_width: dimensions.width,
        responsive_height: dimensions.height,
      });
    },
    300
  );

  const setOverrideUrl = useCallback(
    (url: string) => {
      debouncedSaveUrl(url);
    },
    [debouncedSaveUrl]
  );

  const setCurrentRoute = useCallback(
    (route: string) => {
      void saveSettings({ current_route: route });
    },
    [saveSettings]
  );

  const setScreenSize = useCallback(
    (size: ScreenSize) => {
      saveSettings({ screen_size: size });
    },
    [saveSettings]
  );

  const setResponsiveDimensions = useCallback(
    (dimensions: ResponsiveDimensions) => {
      debouncedSaveDimensions(dimensions);
    },
    [debouncedSaveDimensions]
  );

  const clearOverride = useCallback(async () => {
    cancelSaveUrl();
    await saveSettings({ url: '' });
  }, [cancelSaveUrl, saveSettings]);

  return {
    overrideUrl,
    hasOverride,
    setOverrideUrl,
    clearOverride,
    currentRoute,
    setCurrentRoute,
    screenSize,
    responsiveDimensions,
    setScreenSize,
    setResponsiveDimensions,
    isLoading: isScratchLoading,
  };
}
