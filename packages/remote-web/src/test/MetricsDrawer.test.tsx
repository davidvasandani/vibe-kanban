import { useState } from "react";
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {
  MetricsDrawer,
  METRICS_DRAWER_MAX_WIDTH,
  METRICS_DRAWER_MIN_WIDTH,
} from "@vibe/ui/components/MetricsDrawer";

function Harness({ initialWidth = 420 }: { initialWidth?: number }) {
  const [width, setWidth] = useState(initialWidth);
  const [open, setOpen] = useState(false);
  return (
    <>
      <button type="button" onClick={() => setOpen(true)}>
        opener
      </button>
      <MetricsDrawer
        open={open}
        width={width}
        onWidthChange={setWidth}
        onClose={() => setOpen(false)}
        title="Server metrics"
      >
        <div data-testid="drawer-body">body</div>
      </MetricsDrawer>
    </>
  );
}

describe("MetricsDrawer", () => {
  it("is right-anchored and slides in and out via transform", () => {
    const { rerender } = render(
      <MetricsDrawer
        open={false}
        width={420}
        onWidthChange={vi.fn()}
        onClose={vi.fn()}
        title="Server metrics"
      />,
    );
    const panel = screen.getByRole("dialog", { hidden: true });
    expect(panel.className).toContain("right-0");
    expect(panel.className).toContain("translate-x-full");
    expect(panel.className).toContain("transition-transform");

    rerender(
      <MetricsDrawer
        open={true}
        width={420}
        onWidthChange={vi.fn()}
        onClose={vi.fn()}
        title="Server metrics"
      />,
    );
    expect(screen.getByRole("dialog").className).toContain("translate-x-0");
  });

  it("overlays rather than blocking the app", () => {
    render(
      <MetricsDrawer
        open={true}
        width={420}
        onWidthChange={vi.fn()}
        onClose={vi.fn()}
        title="Server metrics"
      />,
    );
    expect(screen.getByRole("dialog")).toHaveAttribute("aria-modal", "false");
  });

  it("scrolls on one axis only (overflow-x must be pinned)", () => {
    render(
      <MetricsDrawer
        open={true}
        width={420}
        onWidthChange={vi.fn()}
        onClose={vi.fn()}
        title="Server metrics"
      >
        <div data-testid="drawer-body">body</div>
      </MetricsDrawer>,
    );
    const scroller = screen.getByTestId("drawer-body").parentElement;
    expect(scroller?.className).toContain("overflow-y-auto");
    expect(scroller?.className).toContain("overflow-x-hidden");
  });

  it("clamps the rendered width to the drag range", () => {
    const { rerender } = render(
      <MetricsDrawer
        open={true}
        width={10}
        onWidthChange={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByRole("dialog")).toHaveStyle({
      width: `${METRICS_DRAWER_MIN_WIDTH}px`,
    });
    rerender(
      <MetricsDrawer
        open={true}
        width={9000}
        onWidthChange={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByRole("dialog")).toHaveStyle({
      width: `${METRICS_DRAWER_MAX_WIDTH}px`,
    });
  });

  it("closes on Escape and on a backdrop click", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    const { container } = render(
      <MetricsDrawer
        open={true}
        width={420}
        onWidthChange={vi.fn()}
        onClose={onClose}
      />,
    );

    fireEvent.keyDown(document, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);

    const backdrop = container.ownerDocument.querySelector(".bg-black\\/50");
    expect(backdrop).not.toBeNull();
    await user.click(backdrop as Element);
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it("moves focus in on open and restores it on close", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const opener = screen.getByRole("button", { name: "opener" });
    opener.focus();
    expect(document.activeElement).toBe(opener);

    await user.click(opener);
    const panel = screen.getByRole("dialog");
    expect(panel).toHaveFocus();

    fireEvent.keyDown(document, { key: "Escape" });
    expect(opener).toHaveFocus();
  });

  it("resizes with the keyboard and reports the new width upward", async () => {
    const user = userEvent.setup();
    render(<Harness initialWidth={420} />);
    await user.click(screen.getByRole("button", { name: "opener" }));

    const handle = screen.getByTestId("metrics-drawer-resize");
    expect(handle).toHaveAttribute("role", "separator");
    handle.focus();

    // Right-anchored: left widens, right narrows.
    fireEvent.keyDown(handle, { key: "ArrowLeft" });
    expect(screen.getByRole("dialog")).toHaveStyle({ width: "436px" });
    fireEvent.keyDown(handle, { key: "ArrowRight" });
    fireEvent.keyDown(handle, { key: "ArrowRight" });
    expect(screen.getByRole("dialog")).toHaveStyle({ width: "404px" });
  });

  it("has a labelled close control", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(
      <MetricsDrawer
        open={true}
        width={420}
        onWidthChange={vi.fn()}
        onClose={onClose}
      />,
    );
    const buttons = screen.getAllByRole("button");
    expect(buttons).toHaveLength(1);
    expect(buttons[0].getAttribute("aria-label")).toBeTruthy();
    await user.click(buttons[0]);
    expect(onClose).toHaveBeenCalled();
  });
});
