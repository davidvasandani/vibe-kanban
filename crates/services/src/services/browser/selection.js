// Fixed-purpose CDP expression: never accepts caller-supplied JavaScript.
(() => {
  function bounded(text) {
    if (text.length > 65536) throw new Error('selection too long');
    return text;
  }
  let root = document;
  for (let depth = 0; depth < 32; depth++) {
    const element = root.activeElement;
    if (element?.tagName === 'IFRAME' || element?.tagName === 'FRAME') {
      try {
        if (!element.contentDocument) return '';
        root = element.contentDocument;
        continue;
      } catch {
        return '';
      }
    }
    if (element?.shadowRoot?.activeElement) {
      root = element.shadowRoot;
      continue;
    }
    if (element?.tagName === 'INPUT' || element?.tagName === 'TEXTAREA') {
      if (element.type === 'password') return '';
      const start = element.selectionStart;
      const end = element.selectionEnd;
      return typeof start === 'number' && typeof end === 'number'
        ? bounded(element.value.slice(start, end))
        : '';
    }
    const selection = typeof root.getSelection === 'function'
      ? root.getSelection()
      : root.ownerDocument.getSelection();
    return bounded(selection?.toString() ?? '');
  }
  return '';
})()
