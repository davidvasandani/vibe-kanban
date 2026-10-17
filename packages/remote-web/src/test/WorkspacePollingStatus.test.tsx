import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import {
  categorizeWorkspaces,
  WorkspacesSidebar,
  type WorkspacesSidebarWorkspace,
} from "@vibe/ui/components/WorkspacesSidebar";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

const polling: WorkspacesSidebarWorkspace = {
  id: "polling",
  name: "Monitor deployment",
  hasRunningPoller: true,
  hasUnseenActivity: true,
};

beforeEach(() => localStorage.clear());

describe("polling workspace classification", () => {
  it("puts unread polling workspaces exclusively in Polling, preserving order", () => {
    const workspaces = [
      polling,
      { ...polling, id: "second", hasUnseenActivity: false },
      { id: "attention", name: "Review", hasUnseenActivity: true },
      { id: "idle", name: "Idle" },
      { id: "run", name: "Run", isRunning: true },
    ];
    const groups = categorizeWorkspaces(workspaces);
    expect(groups.pollingWorkspaces.map((ws) => ws.id)).toEqual([
      "polling",
      "second",
    ]);
    expect(groups.raisedHandWorkspaces.map((ws) => ws.id)).toEqual([
      "attention",
    ]);
    expect(groups.idleWorkspaces.map((ws) => ws.id)).toEqual(["idle"]);
    expect(groups.runningWorkspaces.map((ws) => ws.id)).toEqual(["run"]);
    expect(Object.values(groups).flat()).toHaveLength(workspaces.length);
    expect(polling.hasUnseenActivity).toBe(true);
  });

  it.each([
    [{ hasPendingApproval: true }, "raisedHandWorkspaces"],
    [{ isRunning: true }, "runningWorkspaces"],
    [{ isCreating: true }, "runningWorkspaces"],
    [{ isRunning: true, hasPendingApproval: true }, "raisedHandWorkspaces"],
    [{ isCreating: true, hasPendingApproval: true }, "raisedHandWorkspaces"],
  ] as const)("preserves priority for %o", (flags, expected) => {
    const workspace = { ...polling, ...flags };
    const groups = categorizeWorkspaces([workspace]);
    expect(groups[expected]).toEqual([workspace]);
    expect(groups.pollingWorkspaces).toEqual([]);
    expect(Object.values(groups).flat()).toHaveLength(1);
  });

  it("restores attention or idle once the last poller ends", () => {
    const stopped = { ...polling, hasRunningPoller: false };
    expect(categorizeWorkspaces([stopped]).raisedHandWorkspaces).toEqual([
      stopped,
    ]);
    const seen = { ...stopped, hasUnseenActivity: false };
    expect(categorizeWorkspaces([seen]).idleWorkspaces).toEqual([seen]);
    expect(
      categorizeWorkspaces([{ ...seen, hasRunningDevServer: true }])
        .pollingWorkspaces,
    ).toEqual([]);
  });

  it("keeps pre-existing grouping for every non-polling signal combination", () => {
    for (let flags = 0; flags < 16; flags++) {
      const workspace = {
        id: "legacy",
        name: "Legacy",
        isRunning: Boolean(flags & 1),
        isCreating: Boolean(flags & 2),
        hasPendingApproval: Boolean(flags & 4),
        hasUnseenActivity: Boolean(flags & 8),
      };
      const needsAttention =
        workspace.hasPendingApproval ||
        (workspace.hasUnseenActivity && !workspace.isRunning);
      const group = needsAttention
        ? "raisedHandWorkspaces"
        : workspace.isRunning || workspace.isCreating
          ? "runningWorkspaces"
          : "idleWorkspaces";
      const groups = categorizeWorkspaces([workspace]);
      expect(groups[group]).toEqual([workspace]);
      expect(groups.pollingWorkspaces).toEqual([]);
      expect(Object.values(groups).flat()).toHaveLength(1);
    }
  });
});

describe("Polling sidebar section", () => {
  const props = {
    workspaces: [polling],
    totalWorkspacesCount: 1,
    selectedWorkspaceId: null,
    onSelectWorkspace: vi.fn(),
    searchQuery: "",
    onSearchChange: vi.fn(),
    layoutMode: "accordion" as const,
  };

  it("renders in order, selects a workspace, and persists collapse independently", () => {
    const { unmount } = render(<WorkspacesSidebar {...props} />);
    const headers = screen.getAllByRole("button").map((el) => el.textContent);
    const sectionTitles = ["needsAttention", "running", "polling", "idle"].map(
      (name) => `common:workspaces.${name}`,
    );
    const positions = sectionTitles.map((title) =>
      headers.findIndex((text) => text?.includes(title)),
    );
    expect(positions.every((position) => position >= 0)).toBe(true);
    expect(positions).toEqual([...positions].sort((a, b) => a - b));
    fireEvent.click(screen.getByText("Monitor deployment"));
    expect(props.onSelectWorkspace).toHaveBeenCalledWith("polling");
    fireEvent.click(
      screen.getByRole("button", { name: "common:workspaces.polling" }),
    );
    expect(screen.queryByText("Monitor deployment")).not.toBeInTheDocument();
    expect(
      localStorage.getItem("vibe.ui.collapsible.workspaces-sidebar-polling"),
    ).toBe("false");
    expect(
      localStorage.getItem("vibe.ui.collapsible.workspaces-sidebar-running"),
    ).toBe("true");
    unmount();
    render(<WorkspacesSidebar {...props} />);
    expect(screen.queryByText("Monitor deployment")).not.toBeInTheDocument();
    fireEvent.click(
      screen.getByRole("button", { name: "common:workspaces.polling" }),
    );
    expect(screen.getByText("Monitor deployment")).toBeInTheDocument();
  });

  it("updates membership after summary refresh and leaves flat/archive layouts intact", () => {
    const { rerender } = render(<WorkspacesSidebar {...props} />);
    const workspace = screen.getByText("Monitor deployment");
    expect(workspace).toBeInTheDocument();
    rerender(
      <WorkspacesSidebar
        {...props}
        workspaces={[{ ...polling, hasRunningPoller: false }]}
      />,
    );
    fireEvent.click(
      screen.getByRole("button", { name: "common:workspaces.needsAttention" }),
    );
    expect(screen.queryByText("Monitor deployment")).not.toBeInTheDocument();
    rerender(<WorkspacesSidebar {...props} layoutMode="flat" />);
    expect(
      screen.queryByText("common:workspaces.polling"),
    ).not.toBeInTheDocument();
    expect(screen.getByText("Monitor deployment")).toBeInTheDocument();
    rerender(
      <WorkspacesSidebar
        {...props}
        showArchive
        archivedWorkspaces={[polling]}
      />,
    );
    expect(
      screen.queryByText("common:workspaces.polling"),
    ).not.toBeInTheDocument();
    expect(screen.getByText("Monitor deployment")).toBeInTheDocument();
  });
});
