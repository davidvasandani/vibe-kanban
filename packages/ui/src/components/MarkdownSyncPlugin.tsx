import { useEffect, useRef } from 'react';
import { useLexicalComposerContext } from '@lexical/react/LexicalComposerContext';
import {
  $convertToMarkdownString,
  $convertFromMarkdownString,
  type Transformer,
} from '@lexical/markdown';
import { $getRoot, $isElementNode, type EditorState } from 'lexical';

type MarkdownSyncPluginProps = {
  value: string;
  onChange?: (markdown: string) => void;
  onEditorStateChange?: (state: EditorState) => void;
  editable: boolean;
  transformers: Transformer[];
};

/**
 * Whether the editor structurally holds any visible content.
 *
 * This is intentionally stricter than "serializes to a non-empty markdown
 * string": some content (e.g. decorator nodes) can round-trip asymmetrically
 * and momentarily serialize to an empty string. A single empty paragraph (the
 * default empty state) counts as no content; anything else — text, or a
 * paragraph containing an inline node such as an image/attachment — counts as
 * content.
 */
function $editorHasContent(): boolean {
  return $getRoot()
    .getChildren()
    .some((child) => !$isElementNode(child) || !child.isEmpty());
}

/**
 * Handles bidirectional markdown synchronization between Lexical editor and external state.
 *
 * Uses an internal ref to prevent infinite update loops during bidirectional sync.
 */
export function MarkdownSyncPlugin({
  value,
  onChange,
  onEditorStateChange,
  editable,
  transformers,
}: MarkdownSyncPluginProps) {
  const [editor] = useLexicalComposerContext();
  const lastSerializedRef = useRef<string | undefined>(undefined);
  const prevTransformersRef = useRef(transformers);

  // Detect transformer changes and force re-parse
  if (transformers !== prevTransformersRef.current) {
    prevTransformersRef.current = transformers;
    lastSerializedRef.current = undefined;
  }

  // Handle editable state
  useEffect(() => {
    editor.setEditable(editable);
  }, [editor, editable]);

  // Handle controlled value changes (external → editor)
  useEffect(() => {
    if (value === lastSerializedRef.current) return;

    try {
      editor.update(() => {
        if (value.trim() === '') {
          $getRoot().clear();
        } else {
          $convertFromMarkdownString(value, transformers);
        }

        // Only position cursor at end if editor already has focus (user is actively editing)
        // This prevents unwanted focus when value changes externally (e.g., panel opening)
        const rootElement = editor.getRootElement();
        if (rootElement?.contains(document.activeElement)) {
          const root = $getRoot();
          const lastNode = root.getLastChild();
          if (lastNode) {
            lastNode.selectEnd();
          }
        }
      });
      lastSerializedRef.current = value;
    } catch (err) {
      console.error('Failed to parse markdown', err);
    }
  }, [editor, value, transformers]);

  // Handle editor changes (editor → external)
  useEffect(() => {
    return editor.registerUpdateListener(({ editorState }) => {
      onEditorStateChange?.(editorState);
      if (!onChange) return;

      const { markdown, hasContent } = editorState.read(() => ({
        markdown: $convertToMarkdownString(transformers),
        hasContent: $editorHasContent(),
      }));

      if (markdown === lastSerializedRef.current) return;

      // Never report empty content while the editor visibly holds content.
      // Otherwise external state (which gates the Send button) would be wiped
      // to '', and the controlled-value effect above would early-return on the
      // next render (value === lastSerializedRef), leaving a prefilled prompt
      // on screen that can no longer be sent.
      if (markdown.trim() === '' && hasContent) return;

      lastSerializedRef.current = markdown;
      onChange(markdown);
    });
  }, [editor, onChange, onEditorStateChange, transformers]);

  return null;
}
