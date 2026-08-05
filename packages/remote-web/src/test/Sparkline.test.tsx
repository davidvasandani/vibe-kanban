import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { Sparkline } from "@vibe/ui/components/Sparkline";
import { Meter } from "@vibe/ui/components/Meter";

// Geometry chosen so the expected coordinates are exact:
//   pad = strokeWidth / 2 = 1, usable height = height - strokeWidth = 20
//   y(0) = 21, y(5) = 11, y(10) = 1
const GEOMETRY = {
  min: 0,
  max: 10,
  width: 100,
  height: 22,
  strokeWidth: 2,
} as const;

function paths() {
  return screen
    .queryAllByTestId("sparkline-segment")
    .map((el) => el.getAttribute("d"));
}

describe("Sparkline", () => {
  it("draws a known series as one polyline with the expected geometry", () => {
    render(<Sparkline label="CPU" values={[0, 5, 10]} {...GEOMETRY} />);
    expect(paths()).toEqual(["M 0,21 L 50,11 L 100,1"]);
  });

  it("breaks the line at a null reading instead of plotting it", () => {
    render(
      <Sparkline label="CPU" values={[0, 5, null, 5, 10]} {...GEOMETRY} />,
    );
    // Two separate polylines: the gap is real, not a segment through zero.
    expect(paths()).toEqual(["M 0,21 L 25,11", "M 75,11 L 100,1"]);
    // No single path spans the missing sample.
    expect(paths().some((d) => d?.includes("50,"))).toBe(false);
  });

  it("never treats a null as a zero reading", () => {
    render(<Sparkline label="CPU" values={[10, null, 10]} {...GEOMETRY} />);
    // Isolated readings render as points, and nothing is drawn at the
    // baseline (y = 21) where a null-as-zero would have landed.
    const points = screen.getAllByTestId("sparkline-point");
    expect(points).toHaveLength(2);
    expect(points.map((p) => p.getAttribute("cy"))).toEqual(["1", "1"]);
    expect(paths()).toEqual([]);
  });

  it("renders an em dash when there is no reading at all", () => {
    render(<Sparkline label="CPU" values={[null, null]} {...GEOMETRY} />);
    expect(screen.getByTestId("sparkline-no-reading")).toHaveTextContent("—");
    expect(paths()).toEqual([]);
  });

  it("exposes a text equivalent via role=img", () => {
    const { container } = render(
      <Sparkline label="CPU" values={[0, 5, 10]} {...GEOMETRY} />,
    );
    const img = container.querySelector('[role="img"]');
    expect(img).not.toBeNull();
    expect(img?.getAttribute("aria-label")).toBeTruthy();
  });
});

describe("Meter", () => {
  it("fills proportionally to the reading", () => {
    render(<Meter label="Memory" value={25} max={100} valueText="25%" />);
    expect(screen.getByTestId("meter-fill")).toHaveStyle({ width: "25%" });
    expect(screen.queryByTestId("meter-no-reading")).not.toBeInTheDocument();
  });

  it("renders an em dash for a null reading, not an empty bar", () => {
    render(<Meter label="Memory" value={null} />);
    expect(screen.getByTestId("meter-no-reading")).toHaveTextContent("—");
    expect(screen.queryByTestId("meter-track")).not.toBeInTheDocument();
    expect(screen.queryByTestId("meter-fill")).not.toBeInTheDocument();
  });
});
