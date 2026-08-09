import { act, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  DeployStatus,
  formatDeployAge,
} from "@vibe/ui/components/DeployStatus";
import { Navbar } from "@vibe/ui/components/Navbar";

afterEach(() => {
  vi.useRealTimers();
});

describe("formatDeployAge", () => {
  const now = Date.parse("2026-08-09T15:00:00Z");

  it.each([
    ["2026-08-09T15:00:00Z", "now"],
    ["2026-08-09T14:59:00Z", "1m"],
    ["2026-08-09T13:00:00Z", "2h"],
    ["2026-08-07T15:00:00Z", "2d"],
    ["2026-07-19T15:00:00Z", "3w"],
  ])("formats %s as %s", (timestamp, expected) => {
    expect(formatDeployAge(timestamp, now)?.compact).toBe(expected);
  });

  it("rejects invalid timestamps and clamps future timestamps", () => {
    expect(formatDeployAge("invalid", now)).toBeNull();
    expect(formatDeployAge("2026-08-10T15:00:00Z", now)?.compact).toBe("now");
  });
});

describe("DeployStatus", () => {
  it("links a deployed revision and advances its elapsed label", () => {
    vi.useFakeTimers();
    vi.setSystemTime("2026-08-09T15:00:00Z");

    render(
      <DeployStatus
        version="abc1234"
        deploymentTimestamp="2026-08-09T14:59:00Z"
      />,
    );

    const link = screen.getByRole("link", {
      name: "Deployed revision abc1234 1 minute ago",
    });
    expect(link).toHaveAttribute(
      "href",
      "https://github.com/davidvasandani/vibe-kanban/commit/abc1234",
    );
    expect(screen.getByText("· 1m")).toHaveClass("min-[390px]:inline");

    act(() => {
      vi.advanceTimersByTime(60_000);
    });

    expect(
      screen.getByRole("link", {
        name: "Deployed revision abc1234 2 minutes ago",
      }),
    ).toBeInTheDocument();
  });

  it("renders dev and missing timestamps without misleading links or age", () => {
    const { rerender } = render(
      <DeployStatus version="dev" deploymentTimestamp="2026-08-09T14:59:00Z" />,
    );

    expect(screen.getByLabelText("Development build")).toHaveTextContent("dev");
    expect(screen.queryByRole("link")).not.toBeInTheDocument();

    rerender(<DeployStatus version="abc1234" deploymentTimestamp={null} />);
    expect(
      screen.getByRole("link", { name: "Deployed revision abc1234" }),
    ).toHaveTextContent("abc1234");
    expect(screen.queryByText(/·/)).not.toBeInTheDocument();
  });
});

describe("mobile Navbar deployment status", () => {
  it("places deployment identity in the mobile header", () => {
    vi.useFakeTimers();
    vi.setSystemTime("2026-08-09T15:00:00Z");

    render(
      <Navbar
        mobileMode
        showMobileTabs={false}
        appVersion="abc1234"
        deploymentTimestamp="2026-08-09T13:00:00Z"
        onOpenSettings={() => {}}
        onOpenCommandBar={() => {}}
      />,
    );

    expect(
      screen.getByRole("link", {
        name: "Deployed revision abc1234 2 hours ago",
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Settings" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Command bar" }),
    ).toBeInTheDocument();
  });
});
