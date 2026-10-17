import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
} from 'react';
import { useTranslation } from 'react-i18next';
import type { BrowserAction, BrowserActionResult } from 'shared/types';

export interface BrowserTextInputHandle {
  focus: () => void;
  copy: () => void;
  paste: (text: string) => void;
}

interface Props {
  sendInput: (action: BrowserAction) => Promise<BrowserActionResult | null>;
}

// Keep one character behind the caret so a soft keyboard can delete even
// though committed text is immediately cleared from the local editor.
const SENTINEL = '\u200b';
const MAX_TEXT_LENGTH = 65536;

export const BrowserTextInput = forwardRef<BrowserTextInputHandle, Props>(
  function BrowserTextInput({ sendInput }, ref) {
    const { t } = useTranslation('tasks');
    const editor = useRef<HTMLTextAreaElement>(null);
    const composing = useRef(false);
    const active = useRef(true);
    const operation = useRef(0);
    const [message, setMessage] = useState<string | null>(null);
    const [manual, setManual] = useState<'copy' | 'paste' | null>(null);
    const [draft, setDraft] = useState('');

    useEffect(() => {
      active.current = true;
      operation.current++;
      return () => {
        active.current = false;
      };
    }, []);

    function resetEditor() {
      if (!editor.current) return;
      editor.current.value = SENTINEL;
      editor.current.setSelectionRange(1, 1);
    }

    function send(action: BrowserAction) {
      if (!active.current) return;
      void sendInput(action).catch(() => {
        if (active.current) setMessage(t('browser.textInput.failed'));
      });
    }

    function paste(text: string) {
      if (!text || !active.current) return false;
      if (text.length > MAX_TEXT_LENGTH) {
        setMessage(t('browser.textInput.tooLong'));
        return false;
      }
      send({ type: 'type', text });
      return true;
    }

    async function copy() {
      const request = ++operation.current;
      setMessage(null);
      setManual(null);
      setDraft('');
      try {
        const result = await sendInput({ type: 'copy_selection' });
        if (!active.current || request !== operation.current) return;
        const text = typeof result?.value === 'string' ? result.value : '';
        if (!text) {
          setMessage(t('browser.textInput.noSelection'));
          return;
        }
        // Keep a selectable fallback: Safari may no longer consider the
        // asynchronous remote response part of the original user gesture.
        setDraft(text);
        setManual('copy');
        try {
          await navigator.clipboard.writeText(text);
          if (!active.current || request !== operation.current) return;
          setManual(null);
          setDraft('');
          setMessage(t('browser.textInput.copied'));
        } catch {
          if (active.current && request === operation.current) {
            setMessage(t('browser.textInput.copyManually'));
          }
        }
      } catch {
        if (active.current && request === operation.current) {
          setMessage(t('browser.textInput.failed'));
        }
      }
    }

    async function readClipboard() {
      const request = ++operation.current;
      setMessage(null);
      setManual(null);
      setDraft('');
      try {
        const text = await navigator.clipboard.readText();
        if (!active.current || request !== operation.current) return;
        paste(text);
      } catch {
        if (!active.current || request !== operation.current) return;
        setDraft('');
        setManual('paste');
        setMessage(t('browser.textInput.pasteManually'));
      }
    }

    useImperativeHandle(ref, () => ({
      focus: () => {
        editor.current?.focus({ preventScroll: true });
        resetEditor();
      },
      copy: () => void copy(),
      paste,
    }));

    function commit(input: HTMLTextAreaElement, inputType?: string) {
      if (composing.current) return;
      const value = input.value;
      const text = value.startsWith(SENTINEL) ? value.slice(1) : value;
      if (inputType?.startsWith('delete')) {
        send({
          type: 'key',
          key: inputType.includes('Forward') ? 'Delete' : 'Backspace',
          modifiers: null,
        });
      } else if (
        inputType === 'insertLineBreak' ||
        inputType === 'insertParagraph'
      ) {
        send({ type: 'key', key: 'Enter', modifiers: null });
      } else if (text) {
        paste(text);
      }
      resetEditor();
    }

    const buttonClass =
      'px-base py-half bg-secondary rounded border text-base text-normal hover:text-high';

    return (
      <div className="shrink-0 border-b p-half flex flex-wrap items-center gap-half">
        <button
          type="button"
          className={buttonClass}
          onClick={() => {
            editor.current?.focus({ preventScroll: true });
            resetEditor();
          }}
        >
          {t('browser.textInput.keyboard')}
        </button>
        <button
          type="button"
          className={buttonClass}
          onClick={() => send({ type: 'key', key: 'a', modifiers: ['ctrl'] })}
        >
          {t('browser.textInput.selectAll')}
        </button>
        <button
          type="button"
          className={buttonClass}
          onClick={() => void copy()}
        >
          {t('browser.textInput.copy')}
        </button>
        <button
          type="button"
          className={buttonClass}
          onClick={() => void readClipboard()}
        >
          {t('browser.textInput.paste')}
        </button>
        <textarea
          ref={editor}
          rows={1}
          defaultValue={SENTINEL}
          aria-label={t('browser.textInput.keyboard')}
          autoCapitalize="off"
          autoComplete="off"
          autoCorrect="off"
          spellCheck={false}
          className="min-w-0 w-20 flex-1 resize-none rounded border bg-secondary text-normal text-[16px] px-half py-half focus:ring-1 focus:ring-brand"
          onFocus={resetEditor}
          onBlur={() => {
            composing.current = false;
            resetEditor();
          }}
          onCompositionStart={() => {
            composing.current = true;
          }}
          onCompositionEnd={(e) => {
            composing.current = false;
            commit(e.currentTarget);
          }}
          onInput={(e) => {
            const event = e.nativeEvent as InputEvent;
            if (!event.isComposing) commit(e.currentTarget, event.inputType);
          }}
          onPaste={(e) => {
            e.preventDefault();
            paste(e.clipboardData.getData('text/plain'));
          }}
          onKeyDown={(e) => {
            if (
              composing.current ||
              e.nativeEvent.isComposing ||
              e.keyCode === 229
            )
              return;
            const shortcut =
              (e.ctrlKey || e.metaKey) &&
              !e.altKey &&
              !e.getModifierState('AltGraph');
            if (shortcut && e.key.toLowerCase() === 'v') return;
            if (shortcut && e.key.toLowerCase() === 'c') {
              e.preventDefault();
              void copy();
              return;
            }
            if (
              e.key === 'Unidentified' ||
              e.key === 'Process' ||
              e.key === 'Dead'
            )
              return;
            if (['Control', 'Meta', 'Shift', 'Alt'].includes(e.key)) return;
            if (!shortcut && e.key.length === 1) return;
            e.preventDefault();
            send({
              type: 'key',
              key: e.key,
              modifiers: [
                ...(e.ctrlKey ? ['ctrl'] : []),
                ...(e.metaKey ? ['meta'] : []),
                ...(e.altKey ? ['alt'] : []),
                ...(e.shiftKey ? ['shift'] : []),
              ],
            });
          }}
        />
        {message && (
          <p role="status" className="w-full text-base text-normal">
            {message}
          </p>
        )}
        {manual && (
          <div className="flex w-full flex-wrap gap-half">
            <textarea
              aria-label={t(
                manual === 'copy'
                  ? 'browser.textInput.copyManually'
                  : 'browser.textInput.pasteManually'
              )}
              className="w-full rounded border bg-secondary text-normal text-[16px] p-half"
              value={draft}
              readOnly={manual === 'copy'}
              onChange={(e) => setDraft(e.target.value)}
              onFocus={(e) => {
                if (manual === 'copy') e.currentTarget.select();
              }}
            />
            {manual === 'paste' && (
              <button
                type="button"
                className={buttonClass}
                onClick={() => {
                  if (paste(draft)) {
                    setDraft('');
                    setManual(null);
                  }
                }}
              >
                {t('browser.textInput.paste')}
              </button>
            )}
            <button
              type="button"
              className={buttonClass}
              onClick={() => {
                operation.current++;
                setDraft('');
                setManual(null);
                setMessage(null);
              }}
            >
              {t('browser.textInput.close')}
            </button>
          </div>
        )}
      </div>
    );
  }
);
