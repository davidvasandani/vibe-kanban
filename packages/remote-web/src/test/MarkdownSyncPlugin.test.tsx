import { describe, it, expect, vi } from "vitest";
import { act, render } from "@testing-library/react";
import { useEffect } from "react";
import { LexicalComposer } from "@lexical/react/LexicalComposer";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { TRANSFORMERS, type Transformer } from "@lexical/markdown";
import {
  DecoratorNode,
  $getRoot,
  $createParagraphNode,
  $createTextNode,
  type LexicalEditor,
  type NodeKey,
  type SerializedLexicalNode,
} from "lexical";
import { MarkdownSyncPlugin } from "@vibe/ui/components/MarkdownSyncPlugin";

/**
 * Minimal decorator node that renders as visible content but whose markdown
 * transformer intentionally fails to serialize (export returns null). This
 * reproduces the class of content that displays in the editor while
 * `$convertToMarkdownString` yields an empty string — the exact condition that
 * used to wipe the Send-button gating state to '' and leave a prefilled prompt
 * unsendable.
 */
class WidgetNode extends DecoratorNode<null> {
  static getType(): string {
    return "test-widget";
  }
  static clone(node: WidgetNode): WidgetNode {
    return new WidgetNode(node.__key);
  }
  static importJSON(): WidgetNode {
    return new WidgetNode();
  }
  constructor(key: NodeKey | undefined = undefined) {
    super(key);
  }
  exportJSON(): SerializedLexicalNode {
    return { type: WidgetNode.getType(), version: 1 };
  }
  createDOM(): HTMLElement {
    return document.createElement("span");
  }
  updateDOM(): false {
    return false;
  }
  isInline(): boolean {
    return true;
  }
  decorate(): null {
    return null;
  }
}

const WIDGET_TRANSFORMER: Transformer = {
  dependencies: [WidgetNode],
  // Simulate a node that cannot be serialized back to markdown.
  export: () => null,
  importRegExp: /@@widget@@/,
  regExp: /@@widget@@$/,
  replace: (textNode) => {
    textNode.replace(new WidgetNode());
  },
  trigger: "@",
  type: "text-match",
};

const transformers: Transformer[] = [WIDGET_TRANSFORMER, ...TRANSFORMERS];

function CaptureEditor({
  onReady,
}: {
  onReady: (editor: LexicalEditor) => void;
}) {
  const [editor] = useLexicalComposerContext();
  useEffect(() => {
    onReady(editor);
  }, [editor, onReady]);
  return null;
}

function renderEditor(value: string, onChange: (md: string) => void) {
  let editor: LexicalEditor | null = null;
  const utils = render(
    <LexicalComposer
      initialConfig={{
        namespace: "test",
        nodes: [WidgetNode],
        onError: (e) => {
          throw e;
        },
      }}
    >
      <CaptureEditor onReady={(e) => (editor = e)} />
      <MarkdownSyncPlugin
        value={value}
        onChange={onChange}
        editable={true}
        transformers={transformers}
      />
    </LexicalComposer>,
  );
  return { ...utils, getEditor: () => editor as LexicalEditor };
}

function emptyOnChangeCalls(onChange: ReturnType<typeof vi.fn>) {
  return onChange.mock.calls.filter(
    ([md]) => typeof md === "string" && md.trim() === "",
  );
}

describe("MarkdownSyncPlugin – Send gating (prefilled prompt)", () => {
  it("does not wipe state to '' when visible content serializes to an empty string", () => {
    const onChange = vi.fn();
    // Prefill with real text so external state is non-empty (Send enabled).
    const { getEditor } = renderEditor("hello world", onChange);
    const editor = getEditor();

    onChange.mockClear();

    // Now the content transitions to a node that renders but serializes empty
    // (e.g. an attachment/widget replacing the text). This fires the
    // editor->state update listener while content is still on screen.
    act(() => {
      editor.update(
        () => {
          const root = $getRoot();
          root.clear();
          const paragraph = $createParagraphNode();
          paragraph.append(new WidgetNode());
          root.append(paragraph);
        },
        { discrete: true },
      );
    });

    // The editor still visibly holds content, so onChange must never be called
    // with an empty string — otherwise Send would grey out while the prompt is
    // still visible and unsendable.
    expect(emptyOnChangeCalls(onChange)).toEqual([]);
  });

  it("reports '' when a non-empty prompt is replaced with whitespace only", () => {
    const onChange = vi.fn();
    const { getEditor } = renderEditor("hello world", onChange);
    const editor = getEditor();

    onChange.mockClear();

    // Replace the text with whitespace only. The paragraph still has a text
    // child, but whitespace serializes to '' and MUST clear the Send state —
    // otherwise the stale "hello world" prompt could be sent.
    act(() => {
      editor.update(
        () => {
          const root = $getRoot();
          root.clear();
          const paragraph = $createParagraphNode();
          paragraph.append($createTextNode("   "));
          root.append(paragraph);
        },
        { discrete: true },
      );
    });

    expect(emptyOnChangeCalls(onChange).length).toBeGreaterThan(0);
  });

  it("still reports '' when the user genuinely clears all content", () => {
    const onChange = vi.fn();
    const { getEditor } = renderEditor("hello world", onChange);
    const editor = getEditor();

    onChange.mockClear();

    // Simulate the user deleting everything in the editor.
    act(() => {
      editor.update(
        () => {
          $getRoot().clear();
        },
        { discrete: true },
      );
    });

    // A genuinely empty editor must report '' so Send correctly disables.
    expect(emptyOnChangeCalls(onChange).length).toBeGreaterThan(0);
  });
});
