import { useEffect, useRef } from 'react';
import { useLexicalComposerContext } from '@lexical/react/LexicalComposerContext';
import {
  $convertToMarkdownString,
  $convertFromMarkdownString,
  type Transformer,
} from '@lexical/markdown';
import {
  $getRoot,
  $isDecoratorNode,
  $isElementNode,
  type EditorState,
  type LexicalNode,
} from 'lexical';

type MarkdownSyncPluginProps = {
  value: string;
  onChange?: (markdown: string) => void;
  onEditorStateChange?: (state: EditorState) => void;
  editable: boolean;
  transformers: Transformer[];
};

/**
 * Whether the editor holds a decorator node (image, attachment, PR comment,
 * component info, …), anywhere in the tree.
 *
 * These render as visible content but can serialize to an empty markdown
 * string, so their presence must not be mistaken for an empty editor. Plain
 * text is deliberately excluded: whitespace-only text serializes to '' and
 * *should* clear the Send state, so it must not be treated as content here.
 */
function $editorHasDecoratorContent(): boolean {
  const hasDecorator = (node: LexicalNode): boolean => {
    if ($isDecoratorNode(node)) return true;
    if ($isElementNode(node)) return node.getChildren().some(hasDecorator);
    return false;
  };
  return $getRoot().getChildren().some(hasDecorator);
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

      const { markdown, hasDecoratorContent } = editorState.read(() => ({
        markdown: $convertToMarkdownString(transformers),
        hasDecoratorContent: $editorHasDecoratorContent(),
      }));

      if (markdown === lastSerializedRef.current) return;

      // Never report empty content while the editor still holds a decorator
      // node that markdown can't serialize. Otherwise external state (which
      // gates the Send button) would be wiped to '', and the controlled-value
      // effect above would early-return on the next render (value ===
      // lastSerializedRef), leaving a prefilled prompt on screen that can no
      // longer be sent. Whitespace-only text is intentionally NOT guarded: it
      // serializes to '' and should correctly clear the Send state.
      if (markdown.trim() === '' && hasDecoratorContent) return;

      lastSerializedRef.current = markdown;
      onChange(markdown);
    });
  }, [editor, onChange, onEditorStateChange, transformers]);

  return null;
}
