import { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import type {
  KeyboardEvent as ReactKeyboardEvent,
  PointerEvent as ReactPointerEvent,
} from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import { CaretLeftIcon, PlusIcon, XIcon } from '@phosphor-icons/react';
import { create, useModal } from '@ebay/nice-modal-react';
import { defineModal } from '@/shared/lib/modals';

import { cn } from '@/shared/lib/utils';
import { useIsMobile } from '@/shared/hooks/useIsMobile';
import {
  clampSettingsDrawerWidth,
  toggleSettingsDrawerWith,
  useSettingsDrawerStore,
  useSettingsDrawerLayout,
} from '@/shared/stores/useSettingsDrawerStore';
import { SettingsSection } from './settings/SettingsSection';
import { SettingsSelect } from './settings/SettingsComponents';
import type {
  SettingsSectionType,
  SettingsSectionInitialState,
} from './settings/SettingsSection';
import {
  SETTINGS_SECTION_DEFINITIONS,
  isHostSpecificSettingsSection,
} from './settings/settingsRegistry';
import {
  SettingsDirtyProvider,
  useSettingsDirty,
} from './settings/SettingsDirtyContext';
import {
  SettingsHostProvider,
  useSettingsHost,
  type SettingsHostTargetId,
} from './settings/SettingsHostContext';
import { SettingsMachineUserSystemProvider } from './settings/SettingsMachineUserSystemProvider';
import { ConfirmDialog } from '@vibe/ui/components/ConfirmDialog';
import { confirmSettingsHostSwitch } from './settings/settingsHostSwitch';

export interface SettingsDialogProps {
  initialSection?: SettingsSectionType;
  initialState?: SettingsSectionInitialState[SettingsSectionType];
  initialHostId?: string | 'local';
  /** Internal: stamped per show() so a repeated deep link re-targets. */
  requestId?: number;
}

interface SettingsDialogContentProps {
  initialSection?: SettingsSectionType;
  initialState?: SettingsSectionInitialState[SettingsSectionType];
  requestId?: number;
  onClose: () => void;
}

function SettingsDialogNavigation({
  activeSection,
  onSectionSelect,
  onHostSelect,
}: {
  activeSection: SettingsSectionType;
  onSectionSelect: (sectionId: SettingsSectionType) => void;
  onHostSelect: (hostId: SettingsHostTargetId) => void;
}) {
  const { t } = useTranslation('settings');
  const { availableHosts, hostsResolved, selectedHost, selectedHostId } =
    useSettingsHost();
  const hostSections = SETTINGS_SECTION_DEFINITIONS.filter(
    (section) => section.group === 'host'
  );
  const universalSections = SETTINGS_SECTION_DEFINITIONS.filter(
    (section) => section.group === 'universal'
  );
  const hostOptions = availableHosts.map((host) => ({
    value: host.id,
    label: host.status != null ? `${host.label} (${host.status})` : host.label,
  }));
  const hostSettingsDisabled = !hostsResolved || !selectedHost;
  const hostHint = !hostsResolved
    ? t('settings.general.loading')
    : availableHosts.length === 0
      ? t('settings.hostPicker.pairMachineHint')
      : t('settings.hostPicker.selectMachineHint');

  const handlePairOtherMachines = () => {
    onSectionSelect('relay');
  };

  const renderSectionButton = (sectionId: SettingsSectionType) => {
    const section = SETTINGS_SECTION_DEFINITIONS.find(
      (item) => item.id === sectionId
    );
    if (!section) return null;
    const Icon = section.icon;
    const isActive = activeSection === section.id;
    const isDisabled =
      isHostSpecificSettingsSection(section.id) && hostSettingsDisabled;
    return (
      <button
        key={section.id}
        type="button"
        onClick={() => onSectionSelect(section.id)}
        disabled={isDisabled}
        aria-disabled={isDisabled}
        className={cn(
          'flex items-center gap-3 text-left px-3 py-2 rounded-sm text-sm transition-colors',
          isDisabled
            ? 'text-low opacity-50 cursor-not-allowed'
            : isActive
              ? 'bg-brand/10 text-brand font-medium'
              : 'text-normal hover:bg-primary/10'
        )}
      >
        <Icon className="size-icon-sm shrink-0" weight="bold" />
        <span className="truncate">
          {t(`settings.layout.nav.${section.id}`)}
        </span>
      </button>
    );
  };

  return (
    <nav className="flex-1 p-2 flex flex-col gap-4 overflow-y-auto">
      <div className="space-y-2">
        <div className="px-3 pt-1">
          <div className="text-[11px] font-semibold uppercase tracking-[0.08em] text-low">
            {t('settings.layout.nav.machineSettings')}
          </div>
        </div>
        <div className="px-2">
          <SettingsSelect
            value={selectedHostId ?? undefined}
            options={hostOptions}
            actions={[
              {
                label: t('settings.layout.nav.pairOtherMachines'),
                icon: PlusIcon,
                onClick: handlePairOtherMachines,
              },
            ]}
            onChange={onHostSelect}
            placeholder={t('settings.layout.nav.selectHost')}
          />
          {hostSettingsDisabled && (
            <p className="mt-2 px-1 text-xs text-low">{hostHint}</p>
          )}
        </div>
        <div className="flex flex-col gap-1">
          {hostSections.map((section) => renderSectionButton(section.id))}
        </div>
      </div>
      <div className="space-y-2">
        <div className="px-3 pt-1">
          <div className="text-[11px] font-semibold uppercase tracking-[0.08em] text-low">
            {t('settings.layout.nav.accountSettings')}
          </div>
        </div>
        <div className="flex flex-col gap-1">
          {universalSections.map((section) => renderSectionButton(section.id))}
        </div>
      </div>
    </nav>
  );
}

function SettingsDialogContent({
  initialSection,
  initialState,
  requestId,
  onClose,
}: SettingsDialogContentProps) {
  const { t } = useTranslation('settings');
  const { isDirty } = useSettingsDirty();
  const {
    availableHosts,
    hostsResolved,
    selectedHost,
    selectedHostId,
    setSelectedHostId,
  } = useSettingsHost();

  const resolvedInitialSection = useMemo<SettingsSectionType>(() => {
    if (
      initialSection &&
      SETTINGS_SECTION_DEFINITIONS.some(
        (section) => section.id === initialSection
      )
    ) {
      return initialSection;
    }

    if (hostsResolved && availableHosts.length === 0) {
      return 'organizations';
    }

    return 'general';
  }, [availableHosts.length, hostsResolved, initialSection]);

  const [activeSection, setActiveSection] = useState<SettingsSectionType>(
    resolvedInitialSection
  );
  // On mobile, null means show the nav menu, a section means show that section
  const [mobileShowContent, setMobileShowContent] = useState<boolean>(
    initialSection === resolvedInitialSection
  );
  const isConfirmingRef = useRef(false);
  // Bumped to remount the active section when a deep link re-targets it.
  const [sectionKey, setSectionKey] = useState(0);
  const drawerRef = useRef<HTMLDivElement>(null);
  const isMobile = useIsMobile();
  const { docked, width: drawerWidth } = useSettingsDrawerLayout(isMobile);
  const setDrawerOpen = useSettingsDrawerStore((s) => s.setOpen);
  const setDrawerWidth = useSettingsDrawerStore((s) => s.setWidth);
  const registerCloseRequest = useSettingsDrawerStore(
    (s) => s.registerCloseRequest
  );

  // Resolves true when there is nothing to lose or the user chose to discard.
  const confirmDiscardIfDirty = useCallback(async (): Promise<boolean> => {
    if (!isDirty) return true;
    const result = await ConfirmDialog.show({
      title: t('settings.unsavedChanges.title'),
      message: t('settings.unsavedChanges.message'),
      confirmText: t('settings.unsavedChanges.discard'),
      cancelText: t('settings.unsavedChanges.cancel'),
      variant: 'destructive',
    });
    return result === 'confirmed';
  }, [isDirty, t]);

  const handleCloseWithConfirmation = useCallback(async () => {
    if (isConfirmingRef.current) return;

    isConfirmingRef.current = true;
    try {
      if (await confirmDiscardIfDirty()) {
        onClose();
      }
    } finally {
      isConfirmingRef.current = false;
    }
  }, [confirmDiscardIfDirty, onClose]);

  const handleSectionSelect = (sectionId: SettingsSectionType) => {
    setActiveSection(sectionId);
    setMobileShowContent(true);
  };

  const handleHostSelect = useCallback(
    async (hostId: SettingsHostTargetId) => {
      if (hostId === selectedHostId || isConfirmingRef.current) {
        return;
      }

      isConfirmingRef.current = true;
      try {
        await confirmSettingsHostSwitch({
          isDirty,
          currentHostId: selectedHostId,
          nextHostId: hostId,
          setSelectedHostId,
          t,
        });
      } finally {
        isConfirmingRef.current = false;
      }
    },
    [isDirty, selectedHostId, setSelectedHostId, t]
  );

  useEffect(() => {
    if (
      hostsResolved &&
      isHostSpecificSettingsSection(activeSection) &&
      availableHosts.length === 0
    ) {
      setActiveSection('organizations');
    }
  }, [activeSection, availableHosts.length, hostsResolved]);

  const handleMobileBack = () => {
    setMobileShowContent(false);
  };

  // The drawer is the close/toggle target for the gear and `G S`.
  useEffect(() => {
    registerCloseRequest(handleCloseWithConfirmation);
  }, [handleCloseWithConfirmation, registerCloseRequest]);

  useEffect(() => {
    setDrawerOpen(true);
    // Focus the drawer so Escape works right after opening; clicking back into
    // the app moves focus out and Escape no longer applies.
    drawerRef.current?.focus({ preventScroll: true });
    return () => {
      setDrawerOpen(false);
      registerCloseRequest(null);
    };
  }, [registerCloseRequest, setDrawerOpen]);

  // A show() while already open re-targets the drawer instead of being lost.
  // Each show() carries a fresh requestId, so repeating the same deep link
  // after navigating inside Settings still re-targets. Leaving a section with
  // unsaved edits goes through the same discard guard as closing.
  const retargetRef = useRef({
    activeSection,
    confirmDiscardIfDirty,
    initialSection,
  });
  retargetRef.current = {
    activeSection,
    confirmDiscardIfDirty,
    initialSection,
  };
  const lastRequestIdRef = useRef(requestId);
  useEffect(() => {
    if (requestId === lastRequestIdRef.current) return;
    lastRequestIdRef.current = requestId;

    const { initialSection: target } = retargetRef.current;
    if (
      !target ||
      !SETTINGS_SECTION_DEFINITIONS.some((section) => section.id === target) ||
      isConfirmingRef.current
    ) {
      drawerRef.current?.focus({ preventScroll: true });
      return;
    }

    void (async () => {
      isConfirmingRef.current = true;
      try {
        if (!(await retargetRef.current.confirmDiscardIfDirty())) return;
        if (retargetRef.current.activeSection === target) {
          // Same section: remount so it picks up the new initial state.
          setSectionKey((key) => key + 1);
        } else {
          setActiveSection(target);
        }
        setMobileShowContent(true);
        drawerRef.current?.focus({ preventScroll: true });
      } finally {
        isConfirmingRef.current = false;
      }
    })();
  }, [requestId]);

  // Escape closes only while focus is inside the drawer itself, so Escape in
  // the chat (or in a menu portaled out of the drawer) leaves Settings open.
  const handleKeyDown = (e: ReactKeyboardEvent<HTMLDivElement>) => {
    if (
      e.key !== 'Escape' ||
      e.defaultPrevented ||
      !(e.target instanceof Node) ||
      !drawerRef.current?.contains(e.target)
    ) {
      return;
    }
    void handleCloseWithConfirmation();
  };

  const handleResizeStart = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.preventDefault();
    const handle = e.currentTarget;
    handle.setPointerCapture(e.pointerId);
    const handleMove = (event: PointerEvent) => {
      setDrawerWidth(
        clampSettingsDrawerWidth(
          window.innerWidth - event.clientX,
          window.innerWidth
        )
      );
    };
    const handleEnd = () => {
      handle.removeEventListener('pointermove', handleMove);
      handle.removeEventListener('pointerup', handleEnd);
      handle.removeEventListener('pointercancel', handleEnd);
    };
    handle.addEventListener('pointermove', handleMove);
    handle.addEventListener('pointerup', handleEnd);
    handle.addEventListener('pointercancel', handleEnd);
  };

  return (
    <div
      ref={drawerRef}
      role="dialog"
      aria-modal={false}
      aria-label={t('settings.layout.nav.title')}
      tabIndex={-1}
      onKeyDown={handleKeyDown}
      className={cn(
        'fixed flex overflow-hidden bg-panel outline-none',
        !docked
          ? // Full-screen sheet: mobile, or a desktop too narrow to dock
            'inset-0 z-[9999] animate-in fade-in-0 slide-in-from-bottom-4 duration-200'
          : // Desktop: docked right drawer. The app shells reserve its width
            // (useSettingsDrawerInset); z sits below dialogs and dropdowns.
            'inset-y-0 right-0 z-[90] border-l border-border shadow-lg animate-in slide-in-from-right-8 duration-200'
      )}
      style={docked ? { width: drawerWidth } : undefined}
    >
      {docked && (
        <div
          role="separator"
          aria-orientation="vertical"
          aria-label={t('settings.layout.resizeDrawer')}
          onPointerDown={handleResizeStart}
          className="absolute inset-y-0 left-0 z-10 w-1.5 cursor-col-resize touch-none transition-colors hover:bg-brand/30 active:bg-brand/40"
        />
      )}
      {/* Sidebar - hidden on mobile when showing content */}
      <div
        className={cn(
          'bg-secondary/80 border-r border-border flex flex-col',
          // Mobile: full width, hidden when showing content
          'w-full',
          mobileShowContent && 'hidden',
          // Desktop: fixed width sidebar, always visible
          'md:w-48 md:shrink-0 md:flex'
        )}
      >
        {/* Header */}
        <div className="p-4 border-b border-border flex items-center justify-between">
          <h2 className="text-lg font-semibold text-high">
            {t('settings.layout.nav.title')}
          </h2>
          {/* Close button - mobile only */}
          <button
            onClick={handleCloseWithConfirmation}
            className="p-1 rounded-sm hover:bg-secondary text-low hover:text-normal md:hidden"
          >
            <XIcon className="size-icon-sm" weight="bold" />
          </button>
        </div>
        <SettingsDialogNavigation
          activeSection={activeSection}
          onSectionSelect={handleSectionSelect}
          onHostSelect={handleHostSelect}
        />
      </div>
      {/* Content - hidden on mobile when showing nav */}
      <div
        className={cn(
          'min-w-0 w-full flex-1 flex flex-col relative overflow-hidden',
          // Mobile: full width, hidden when showing nav
          !mobileShowContent && 'hidden',
          // Desktop: always visible
          'md:flex'
        )}
      >
        {/* Mobile header with back button */}
        <div className="flex items-center gap-2 p-3 border-b border-border md:hidden">
          <button
            onClick={handleMobileBack}
            className="p-1 rounded-sm hover:bg-secondary text-low hover:text-normal"
          >
            <CaretLeftIcon className="size-icon-sm" weight="bold" />
          </button>
          <span className="text-sm font-medium text-high">
            {t(`settings.layout.nav.${activeSection}`)}
          </span>
          <button
            onClick={handleCloseWithConfirmation}
            className="ml-auto p-1 rounded-sm hover:bg-secondary text-low hover:text-normal"
          >
            <XIcon className="size-icon-sm" weight="bold" />
          </button>
        </div>
        {/* Section content */}
        <div className="min-w-0 flex-1 overflow-x-hidden overflow-y-auto">
          {isHostSpecificSettingsSection(activeSection) ? (
            selectedHost ? (
              <SettingsMachineUserSystemProvider key={selectedHostId}>
                <SettingsSection
                  key={sectionKey}
                  type={activeSection}
                  onClose={handleCloseWithConfirmation}
                  initialState={initialState}
                />
              </SettingsMachineUserSystemProvider>
            ) : !hostsResolved ? (
              <div className="px-6 py-8 text-sm text-low">
                {t('settings.general.loading')}
              </div>
            ) : availableHosts.length > 0 ? (
              <div className="px-6 py-8 text-sm text-low">
                {t('settings.hostPicker.selectMachineHint')}
              </div>
            ) : (
              <div className="px-6 py-8 text-sm text-low">
                {t('settings.hostPicker.noHostAvailable')}
              </div>
            )
          ) : (
            <SettingsSection
              key={sectionKey}
              type={activeSection}
              onClose={handleCloseWithConfirmation}
              initialState={initialState}
            />
          )}
        </div>
      </div>
    </div>
  );
}

const SettingsDialogImpl = create<SettingsDialogProps>(
  ({ initialSection, initialState, initialHostId, requestId }) => {
    const modal = useModal();
    const handleClose = useCallback(() => {
      modal.hide();
      modal.resolve();
      modal.remove();
    }, [modal]);

    if (!modal.visible) {
      return null;
    }

    return createPortal(
      <SettingsDirtyProvider>
        <SettingsHostProvider initialHostId={initialHostId}>
          <SettingsDialogContent
            initialSection={initialSection}
            initialState={initialState}
            requestId={requestId}
            onClose={handleClose}
          />
        </SettingsHostProvider>
      </SettingsDirtyProvider>,
      document.body
    );
  }
);

export const SettingsDialog = defineModal<SettingsDialogProps | void, void>(
  SettingsDialogImpl
);

// Stamp every show() so an open drawer can tell a repeated deep link (same
// section and state) from a re-render, and re-target on it.
let settingsShowRequestSeq = 0;
const showSettingsDialog = SettingsDialog.show;
SettingsDialog.show = (props) =>
  showSettingsDialog({
    ...(props as SettingsDialogProps | undefined),
    requestId: ++settingsShowRequestSeq,
  });

/**
 * Open the Settings drawer, or close it (through the unsaved-changes guard)
 * when it is already open. Used by the gear and the `G S` shortcut; deep links
 * keep calling `SettingsDialog.show(...)`, which re-targets an open drawer.
 */
export async function toggleSettingsDrawer(
  props?: SettingsDialogProps
): Promise<void> {
  await toggleSettingsDrawerWith(() => SettingsDialog.show(props));
}
