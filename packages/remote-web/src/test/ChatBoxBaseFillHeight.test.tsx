import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { ChatBoxBase, VisualVariant } from "@vibe/ui/components/ChatBoxBase";

// ChatBoxBase's fillHeight contract has one shrink target — the editor slot.
// Every other row must be shrink-0, or the height deficit is split and the
// footer config row can be pushed out of the host's height. CreateChatBox does
// not currently pass a banner, so this pins the guarantee at the boundary that
// makes it rather than relying on a caller-side invariant.

function renderBase(fillHeight: boolean) {
  render(
    <ChatBoxBase
      editor={<div data-testid="editor" />}
      footerRight={<button type="button">Create</button>}
      banner={<div data-testid="banner">queued</div>}
      visualVariant={VisualVariant.NORMAL}
      fillHeight={fillHeight}
    />,
  );
}

describe("ChatBoxBase – fillHeight leaves exactly one shrink target", () => {
  it("boxes the banner as shrink-0 when filling", () => {
    renderBase(true);

    expect(screen.getByTestId("banner").parentElement).toHaveClass("shrink-0");
  });

  it("renders the banner bare when not filling, keeping the caller's DOM shape", () => {
    renderBase(false);

    expect(screen.getByTestId("banner").parentElement).not.toHaveClass(
      "shrink-0",
    );
  });

  it("leaves the editor slot as the only shrinkable row when filling", () => {
    renderBase(true);

    const slot = screen.getByTestId("chat-box-editor-slot");
    expect(slot).toHaveClass("min-h-[3rem]", "overflow-y-auto");
    expect(slot).not.toHaveClass("shrink-0");

    expect(screen.getByTestId("chat-box-footer")).toHaveClass("shrink-0");
    expect(screen.getByTestId("banner").parentElement).toHaveClass("shrink-0");
  });
});
