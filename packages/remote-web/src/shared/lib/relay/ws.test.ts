import { describe, expect, it, vi } from "vitest";

import { forwardDecodedRelayClose } from "./ws";

describe("relay WebSocket close forwarding", () => {
  it("preserves server 1011 diagnostics without originating a reserved browser close", () => {
    const rawClose = vi.fn();
    const emitClose = vi.fn();

    forwardDecodedRelayClose(
      { close: rawClose },
      1011,
      "execution process stream requires resnapshot",
      emitClose,
    );

    expect(emitClose).toHaveBeenCalledWith(
      1011,
      "execution process stream requires resnapshot",
      false,
    );
    expect(rawClose).toHaveBeenCalledWith();
  });

  it("preserves clean semantics for a normal relay closure", () => {
    const emitClose = vi.fn();
    forwardDecodedRelayClose({ close: vi.fn() }, 1000, "finished", emitClose);
    expect(emitClose).toHaveBeenCalledWith(1000, "finished", true);
  });
});
