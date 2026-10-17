import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { CreateChatBox } from "@vibe/ui/components/CreateChatBox";

// The create-workspace composer is rendered into hosts that supply a definite,
// clipped height (the workspaces layout and the project sidebar's Create
// Workspace panel). Its footer carries the configuration controls and the
// create action, and those must stay inside that height at every viewport size
// — see specs/vk/7b85-new-workspace-co/contracts/layout-contract.md and
// constitution XXXVII.
//
// JSDOM calculates no layout, scroll height, or pixel overflow, so these tests
// assert the structural flex contract that produces the browser-relevant
// behaviour rather than geometry. Browser verification is still required for
// the pixel-level outcome.

function renderCreateChatBox(
  overrides: {
    fillHeight?: boolean;
    error?: string | null;
    isSending?: boolean;
  } = {},
) {
  render(
    <CreateChatBox
      editor={{ value: "a prompt", onChange: vi.fn() }}
      renderEditor={() => <div data-testid="editor" />}
      onSend={vi.fn()}
      isSending={overrides.isSending ?? false}
      executor={{
        selected: "CLAUDE_CODE",
        options: ["CLAUDE_CODE"],
        onChange: vi.fn(),
      }}
      error={overrides.error ?? null}
      modelSelector={<div data-testid="model-selector" />}
      onEditRepos={vi.fn()}
      repoSummaryLabel="2 repositories selected"
      repoSummaryTitle="two repos"
      {...(overrides.fillHeight ? { fillHeight: true } : {})}
    />,
  );
}

const editorSlot = () => screen.getByTestId("chat-box-editor-slot");
const footer = () => screen.getByTestId("chat-box-footer");
const chatBoxRoot = () => footer().parentElement!.parentElement!;

describe("CreateChatBox – the config row stays inside the host's height", () => {
  it("makes the chat box shrinkable within a height-constrained parent", () => {
    renderCreateChatBox({ fillHeight: true });

    expect(chatBoxRoot()).toHaveClass("min-h-0");
    expect(chatBoxRoot()).toHaveClass("flex-col");
  });

  it("routes the deficit to the editor slot, which owns the scroll and keeps a floor", () => {
    renderCreateChatBox({ fillHeight: true });

    // The slot owns the scroll and stops at a usable floor rather than zero: a
    // composer shrunk to nothing is no more usable than a hidden footer, and
    // past the floor the shell scrolls so the footer stays reachable.
    expect(editorSlot()).toHaveClass("min-h-[3rem]");
    expect(editorSlot()).toHaveClass("overflow-y-auto");

    // The zero minimum belongs to the items between the root and the slot:
    // without it their automatic minimum block size refuses to shrink and the
    // deficit stalls above the slot, landing on the footer instead.
    expect(editorSlot().parentElement).toHaveClass("min-h-0");
    expect(chatBoxRoot()).toHaveClass("min-h-0");
  });

  it("never shrinks the footer config row", () => {
    renderCreateChatBox({ fillHeight: true });

    expect(footer()).toHaveClass("shrink-0");
  });

  it("keeps the header out of the shrink path", () => {
    renderCreateChatBox({ fillHeight: true });

    const header = screen
      .getByText("2 repositories selected")
      .closest("[data-testid='chat-box-footer']");
    // The repo summary lives in the footer, not the header.
    expect(header).not.toBeNull();

    const headerRow = chatBoxRoot().querySelector("div.border-b");
    expect(headerRow).toHaveClass("shrink-0");
  });

  it("keeps the config row outside the scrolling region", () => {
    renderCreateChatBox({ fillHeight: true });

    // The whole point: scrolling the prompt can never carry the config row or
    // the create action out of view, because they are not inside the scroller.
    expect(editorSlot()).toContainElement(screen.getByTestId("editor"));
    expect(editorSlot()).not.toContainElement(footer());
    expect(editorSlot()).not.toContainElement(
      screen.getByTestId("model-selector"),
    );
  });
});

describe("CreateChatBox – transient state is not a layout authority", () => {
  it("keeps the config row visible and unshrinkable while an error is shown", () => {
    renderCreateChatBox({
      fillHeight: true,
      error: "Add at least one repository to create a workspace",
    });

    expect(
      screen.getByText("Add at least one repository to create a workspace"),
    ).toBeInTheDocument();
    expect(footer()).toHaveClass("shrink-0");
    expect(editorSlot()).toHaveClass("min-h-[3rem]", "overflow-y-auto");
  });

  it("does not let the error alert take height from the config row", () => {
    renderCreateChatBox({ fillHeight: true, error: "boom" });

    const alert = screen.getByText("boom").parentElement;
    expect(alert).toHaveClass("shrink-0");
  });

  it("keeps the config row visible while the create action is in flight", () => {
    renderCreateChatBox({ fillHeight: true, isSending: true });

    expect(footer()).toHaveClass("shrink-0");
    expect(editorSlot()).toHaveClass("min-h-[3rem]", "overflow-y-auto");
  });
});

describe("CreateChatBox – fill behaviour is opt-in", () => {
  it("applies none of the fill-mode classes by default", () => {
    renderCreateChatBox();

    // SessionChatBox shares ChatBoxBase and is intrinsically sized at the
    // bottom of a conversation; it must not inherit any of this.
    expect(editorSlot().className).toBe("");
    expect(footer()).not.toHaveClass("shrink-0");
    expect(chatBoxRoot()).not.toHaveClass("min-h-0");
  });

  it("still renders the editor inside the slot so the DOM shape is stable", () => {
    renderCreateChatBox();

    expect(editorSlot()).toContainElement(screen.getByTestId("editor"));
  });
});
