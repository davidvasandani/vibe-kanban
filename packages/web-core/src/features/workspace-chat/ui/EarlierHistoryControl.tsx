import { useTranslation } from 'react-i18next';

interface EarlierHistoryControlProps {
  isLoading: boolean;
  error: string | null;
  onLoad: () => void;
}

interface LoadingPresentationProps {
  hidden?: boolean;
  label: string;
}

function LoadingPresentation({
  hidden = false,
  label,
}: LoadingPresentationProps) {
  const pulseClass = hidden ? '' : 'animate-pulse';

  return (
    <div
      aria-hidden={hidden || undefined}
      data-earlier-history-sizer={hidden || undefined}
      className={`col-start-1 row-start-1 flex w-full flex-col items-center gap-2 ${hidden ? 'invisible' : ''}`}
    >
      <div className="flex w-full max-w-md flex-col gap-1.5">
        <div className="flex items-center gap-2">
          <div
            className={`h-2.5 w-16 rounded-full bg-foreground/10 ${pulseClass}`}
          />
          <div
            className={`h-2.5 flex-1 rounded-full bg-foreground/[0.06] ${pulseClass}`}
          />
        </div>
        <div className="flex items-center gap-2">
          <div
            className={`h-2.5 w-24 rounded-full bg-foreground/[0.07] ${pulseClass}`}
            style={{ animationDelay: '150ms' }}
          />
          <div
            className={`h-2.5 w-32 rounded-full bg-foreground/[0.05] ${pulseClass}`}
            style={{ animationDelay: '150ms' }}
          />
        </div>
      </div>
      <span className="text-xs text-low" role={hidden ? undefined : 'status'}>
        {label}
      </span>
    </div>
  );
}

/**
 * Keeps pagination feedback from changing the normal-flow height above the
 * conversation while a page request is in flight. Hidden, noninteractive
 * copies of both states reserve their responsive intrinsic size; the active
 * state retains normal focus and accessibility semantics.
 */
export function EarlierHistoryControl({
  isLoading,
  error,
  onLoad,
}: EarlierHistoryControlProps) {
  const { t } = useTranslation('common');
  const loadingLabel = t('conversation.loadingEarlierMessages');
  const buttonLabel = error
    ? t('conversation.retryEarlierMessages', {
        defaultValue: 'Retry loading earlier messages',
      })
    : t('conversation.loadEarlierMessages', {
        defaultValue: 'Load earlier messages',
      });
  const errorLabel = t('conversation.loadEarlierMessagesError', {
    defaultValue: 'Earlier messages could not be loaded.',
  });
  const showError = Boolean(error) && !isLoading;

  return (
    <div
      data-earlier-history-control
      className="flex flex-col items-center gap-2 px-double py-3"
    >
      <div className="grid w-full place-items-center">
        <LoadingPresentation hidden label={loadingLabel} />
        <span
          aria-hidden="true"
          data-earlier-history-sizer
          className="invisible col-start-1 row-start-1 rounded px-base py-half text-xs"
        >
          {buttonLabel}
        </span>

        {isLoading ? (
          <LoadingPresentation label={loadingLabel} />
        ) : (
          <button
            type="button"
            className="col-start-1 row-start-1 rounded px-base py-half text-xs text-low hover:text-normal focus:outline-none focus:ring-1 focus:ring-brand"
            onClick={onLoad}
          >
            {buttonLabel}
          </button>
        )}
      </div>
      <span
        data-earlier-history-error
        aria-hidden={!showError || undefined}
        className={`text-xs text-error ${showError ? '' : 'invisible'}`}
        role={showError ? 'status' : undefined}
      >
        {errorLabel}
      </span>
    </div>
  );
}
