import { type ReactNode } from 'react';
import { ImageIcon } from '@phosphor-icons/react';
import { useTranslation } from 'react-i18next';
import { cn } from '../lib/cn';
import { Toolbar } from './Toolbar';

export enum VisualVariant {
  NORMAL = 'NORMAL',
  FEEDBACK = 'FEEDBACK',
  EDIT = 'EDIT',
  PLAN = 'PLAN',
}

export interface DropzoneProps {
  getRootProps: () => Record<string, unknown>;
  getInputProps: () => Record<string, unknown>;
  isDragActive: boolean;
}

interface ChatBoxBaseProps {
  // Editor node (provided by frontend)
  editor: ReactNode;

  // Error display
  error?: string | null;

  // Header content (right side - session/executor dropdown)
  headerRight?: ReactNode;

  // Header content (left side - stats)
  headerLeft?: ReactNode;

  // Footer left content (additional toolbar items like attach button)
  footerLeft?: ReactNode;

  // Footer right content (action buttons)
  footerRight: ReactNode;

  // Model selector node (rendered with footer controls)
  modelSelector?: ReactNode;

  // Banner content (queued message indicator, feedback mode indicator)
  banner?: ReactNode;

  // visualVariant
  visualVariant: VisualVariant;

  // Whether the workspace is running (shows animated border)
  isRunning?: boolean;

  // Dropzone props for drag-and-drop image uploads
  dropzone?: DropzoneProps;

  // Fill and shrink within a height-constrained parent instead of sizing to
  // content. The editor slot becomes the sole scroll owner, and the header,
  // banner, error and footer rows stop shrinking so the footer controls stay
  // inside the host's height. Off by default: SessionChatBox sits at the bottom
  // of a conversation and is intrinsically sized.
  fillHeight?: boolean;
}

/**
 * Base chat box layout component.
 * Provides shared structure for CreateChatBox and SessionChatBox.
 */
export function ChatBoxBase({
  editor,
  error,
  headerRight,
  headerLeft,
  footerLeft,
  footerRight,
  modelSelector,
  banner,
  visualVariant,
  isRunning,
  dropzone,
  fillHeight = false,
}: ChatBoxBaseProps) {
  const { t } = useTranslation(['common', 'tasks']);

  const isDragActive = dropzone?.isDragActive ?? false;

  return (
    <div
      {...(dropzone?.getRootProps() ?? {})}
      className={cn(
        'relative flex w-chat max-w-full flex-col rounded-sm border border-border bg-secondary',
        fillHeight && 'min-h-0',
        (visualVariant === VisualVariant.FEEDBACK ||
          visualVariant === VisualVariant.EDIT ||
          visualVariant === VisualVariant.PLAN) &&
          'border-brand bg-brand/10',
        isRunning && 'chat-box-running'
      )}
    >
      {dropzone && <input {...dropzone.getInputProps()} />}

      {isDragActive && (
        <div className="absolute inset-0 z-50 flex items-center justify-center rounded-sm border-2 border-dashed border-brand bg-primary/80 backdrop-blur-sm pointer-events-none animate-in fade-in-0 duration-150">
          <div className="text-center">
            <div className="mx-auto mb-2 w-10 h-10 rounded-full bg-brand/10 flex items-center justify-center">
              <ImageIcon className="h-5 w-5 text-brand" />
            </div>
            <p className="text-sm font-medium text-high">
              {t('tasks:dropzone.dropImagesHere')}
            </p>
            <p className="text-xs text-low mt-0.5">
              {t('tasks:dropzone.supportedFormats')}
            </p>
          </div>
        </div>
      )}
      {/* Error alert */}
      {error && (
        <div
          className={cn(
            'bg-error/10 border-b px-double py-base',
            fillHeight && 'shrink-0'
          )}
        >
          <p className="text-error text-sm">{error}</p>
        </div>
      )}

      {/*
        Banner content (queued indicator, feedback mode, etc.). Boxed when
        filling so it cannot become a second shrink target and steal height from
        the footer; rendered bare otherwise, so the non-filling path keeps the
        caller's own box and DOM shape.
      */}
      {fillHeight ? banner && <div className="shrink-0">{banner}</div> : banner}

      {/* Header - Stats and selector */}
      {visualVariant === VisualVariant.NORMAL && (
        <div
          className={cn(
            'flex items-center gap-base border-b px-base py-base',
            fillHeight && 'shrink-0'
          )}
        >
          <div className="flex flex-1 items-center gap-base text-sm min-w-0 overflow-hidden">
            {headerLeft}
          </div>
          <Toolbar className="gap-[9px]">{headerRight}</Toolbar>
        </div>
      )}

      {/* Editor area */}
      <div
        className={cn(
          'flex flex-col gap-plusfifty px-base py-base rounded-md',
          fillHeight && 'min-h-0'
        )}
      >
        {/*
          The editor slot is rendered unconditionally so the editor area keeps
          the same flex-child count — and therefore the same `gap-plusfifty` —
          in both height modes. When filling, it is the sole scroll owner and the
          sole shrink target. Its floor is deliberate: the zero minimum belongs
          on the intermediate items so the deficit reaches this slot, but the
          slot itself stops at a couple of lines, because a composer shrunk to
          nothing is no more usable than a hidden footer. Past that floor the
          shell scrolls, so the footer stays reachable either way.
        */}
        <div
          data-testid="chat-box-editor-slot"
          className={cn(fillHeight && 'min-h-[3rem] overflow-y-auto')}
        >
          {editor}
        </div>

        {/* Footer - Controls. Must stay inside the host's height. */}
        <div
          data-testid="chat-box-footer"
          className={cn(
            'flex items-end justify-between gap-base',
            fillHeight && 'shrink-0'
          )}
        >
          <Toolbar className="flex-1 min-w-0 flex-wrap !gap-half">
            {modelSelector}
            {footerLeft}
          </Toolbar>
          <div className="flex shrink-0 gap-base">{footerRight}</div>
        </div>
      </div>
    </div>
  );
}
