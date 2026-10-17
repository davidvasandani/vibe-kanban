// @vitest-environment jsdom
import { act, createRef } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  BrowserTextInput,
  type BrowserTextInputHandle,
} from './BrowserTextInput';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

describe('browser keyboard and clipboard', () => {
  let container: HTMLDivElement;
  let root: Root;
  let send: ReturnType<typeof vi.fn>;
  let ref: ReturnType<typeof createRef<BrowserTextInputHandle>>;
  let clipboard: {
    readText: ReturnType<typeof vi.fn>;
    writeText: ReturnType<typeof vi.fn>;
  };

  beforeEach(() => {
    globalThis.IS_REACT_ACT_ENVIRONMENT = true;
    container = document.createElement('div');
    document.body.append(container);
    root = createRoot(container);
    send = vi.fn().mockResolvedValue(null);
    ref = createRef();
    clipboard = {
      readText: vi.fn(),
      writeText: vi.fn().mockResolvedValue(undefined),
    };
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: clipboard,
    });
    act(() => root.render(<BrowserTextInput ref={ref} sendInput={send} />));
  });
  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    globalThis.IS_REACT_ACT_ENVIRONMENT = false;
  });
  function editor() {
    return container.querySelector('textarea')!;
  }
  async function click(label: string) {
    await act(async () => {
      [...container.querySelectorAll('button')]
        .find((button) => button.textContent === `browser.textInput.${label}`)!
        .click();
    });
  }
  function input(value: string, inputType = 'insertText', isComposing = false) {
    act(() => {
      editor().value = value;
      editor().dispatchEvent(
        new InputEvent('input', { bubbles: true, inputType, isComposing })
      );
    });
  }
  it('focuses a real editable element synchronously and forwards Unicode once', () => {
    act(() => ref.current!.focus());
    expect(document.activeElement).toBe(editor());
    input('\u200bhello 🌍');
    expect(send).toHaveBeenCalledTimes(1);
    expect(send).toHaveBeenCalledWith({
      type: 'type',
      text: 'hello 🌍',
    });
    expect(editor().value).toBe('\u200b');
  });
  it('waits for composition and ignores the trailing input after commit', () => {
    act(() =>
      editor().dispatchEvent(
        new CompositionEvent('compositionstart', { bubbles: true })
      )
    );
    input('\u200b日本', 'insertCompositionText', true);
    expect(send).not.toHaveBeenCalled();
    act(() =>
      editor().dispatchEvent(
        new CompositionEvent('compositionend', { bubbles: true, data: '日本' })
      )
    );
    input(editor().value, 'insertText');
    expect(send).toHaveBeenCalledTimes(1);
    expect(send).toHaveBeenCalledWith({
      type: 'type',
      text: '日本',
    });
  });
  it('forwards soft deletion and hardware Enter without duplicating printable keys', () => {
    input('', 'deleteContentBackward');
    expect(send).toHaveBeenLastCalledWith({
      type: 'key',
      key: 'Backspace',
      modifiers: null,
    });
    act(() =>
      editor().dispatchEvent(
        new KeyboardEvent('keydown', {
          bubbles: true,
          key: 'Enter',
          cancelable: true,
        })
      )
    );
    expect(send).toHaveBeenLastCalledWith({
      type: 'key',
      key: 'Enter',
      modifiers: [],
    });
    const count = send.mock.calls.length;
    act(() =>
      editor().dispatchEvent(
        new KeyboardEvent('keydown', { bubbles: true, key: 'a' })
      )
    );
    expect(send).toHaveBeenCalledTimes(count);
  });
  it('lets AltGr and Option produce native text without forwarding shortcuts', () => {
    for (const [key, ctrlKey] of [
      ['@', true],
      ['å', false],
    ] as const) {
      const event = new KeyboardEvent('keydown', {
        bubbles: true,
        cancelable: true,
        key,
        ctrlKey,
        altKey: true,
      });
      act(() => editor().dispatchEvent(event));
      expect(event.defaultPrevented).toBe(false);
      input('\u200b' + key);
    }
    expect(send.mock.calls.map(([action]) => action)).toEqual([
      { type: 'type', text: '@' },
      { type: 'type', text: 'å' },
    ]);
  });
  it('forwards soft keyboard Enter as a remote key', () => {
    input('\u200b\n', 'insertLineBreak');
    expect(send).toHaveBeenCalledWith({
      type: 'key',
      key: 'Enter',
      modifiers: null,
    });
  });
  it('pastes plain text from a trusted paste event without clipboard API access', () => {
    const event = new Event('paste', { bubbles: true, cancelable: true });
    Object.defineProperty(event, 'clipboardData', {
      value: {
        getData: (format: string) =>
          format === 'text/plain' ? 'a\nb🙂' : '<b>wrong</b>',
      },
    });
    act(() => editor().dispatchEvent(event));
    expect(send).toHaveBeenCalledTimes(1);
    expect(send).toHaveBeenCalledWith({
      type: 'type',
      text: 'a\nb🙂',
    });
    expect(clipboard.readText).not.toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(true);
  });
  it('copies selected remote text and falls back to a selectable editor on denial', async () => {
    send.mockResolvedValue({ value: 'remote selection' });
    clipboard.writeText.mockRejectedValue(new Error('denied'));
    await click('copy');
    expect(send).toHaveBeenCalledWith({ type: 'copy_selection' });
    expect(clipboard.writeText).toHaveBeenCalledWith('remote selection');
    const manual = container.querySelectorAll('textarea')[1];
    expect(manual.value).toBe('remote selection');
    expect(manual.readOnly).toBe(true);
  });
  it('provides a manual paste editor on denied reads', async () => {
    clipboard.readText.mockRejectedValue(new Error('denied'));
    await click('paste');
    expect(container.querySelectorAll('textarea')).toHaveLength(2);
    expect(container.textContent).toContain('browser.textInput.pasteManually');
    expect(send).not.toHaveBeenCalled();
  });
  it('discards a clipboard read when control/session replaces the component', async () => {
    let resolve!: (value: string) => void;
    clipboard.readText.mockReturnValue(
      new Promise<string>((r) => {
        resolve = r;
      })
    );
    await click('paste');
    act(() =>
      root.render(<BrowserTextInput key="new-session" sendInput={send} />)
    );
    await act(async () => resolve('old clipboard'));
    expect(send).not.toHaveBeenCalled();
  });
  it('discards a remote copy result after ownership is lost', async () => {
    let resolve!: (value: unknown) => void;
    send.mockReturnValue(
      new Promise((r) => {
        resolve = r;
      })
    );
    await click('copy');
    act(() => root.render(null));
    await act(async () => resolve({ value: 'old selection' }));
    expect(clipboard.writeText).not.toHaveBeenCalled();
  });
  it('bounds text and does not send empty input', () => {
    act(() => ref.current!.paste(''));
    act(() => ref.current!.paste('x'.repeat(65537)));
    expect(send).not.toHaveBeenCalled();
    expect(container.textContent).toContain('browser.textInput.tooLong');
  });
});
