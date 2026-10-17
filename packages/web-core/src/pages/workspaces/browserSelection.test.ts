// @vitest-environment jsdom
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';

// Exercise the exact fixed expression shipped in the CDP driver.
const expression = readFileSync(
  resolve(
    __dirname,
    '../../../../../crates/services/src/services/browser/selection.js'
  ),
  'utf8'
);
const selection = () => window.eval(expression) as string;
afterEach(() => {
  document.body.innerHTML = '';
  window.getSelection()?.removeAllRanges();
});

describe('remote selection expression', () => {
  it('copies only the selected input substring and excludes passwords', () => {
    const input = document.createElement('input');
    input.value = 'hello 🌍 world';
    document.body.append(input);
    input.focus();
    input.setSelectionRange(6, 8);
    expect(selection()).toBe('🌍');
    input.type = 'password';
    expect(selection()).toBe('');
  });
  it('copies multiline textarea selection', () => {
    const input = document.createElement('textarea');
    input.value = 'first\nsecond';
    document.body.append(input);
    input.focus();
    input.select();
    expect(selection()).toBe('first\nsecond');
  });
  it('copies document selection', () => {
    const paragraph = document.createElement('p');
    paragraph.textContent = 'Selected page text';
    document.body.append(paragraph);
    const range = document.createRange();
    range.selectNodeContents(paragraph);
    window.getSelection()!.addRange(range);
    expect(selection()).toBe('Selected page text');
  });
  it('follows focus into open shadow roots', () => {
    const host = document.createElement('div');
    document.body.append(host);
    const shadow = host.attachShadow({ mode: 'open' });
    const input = document.createElement('input');
    shadow.append(input);
    input.value = 'shadow selection';
    input.focus();
    input.select();
    expect(selection()).toBe('shadow selection');
  });
});
