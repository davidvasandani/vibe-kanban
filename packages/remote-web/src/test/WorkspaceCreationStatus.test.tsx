import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { WorkspaceSummary } from "@vibe/ui/components/WorkspaceSummary";
import {
  categorizeWorkspaces,
  type WorkspacesSidebarWorkspace,
} from "@vibe/ui/components/WorkspacesSidebar";

describe("workspace creation status", () => {
  it("shows an in-progress status before the workspace starts running", () => {
    render(<WorkspaceSummary name="New workspace" isCreating />);

    expect(screen.getByRole("status")).toHaveTextContent(
      /workspaces\.creating|Creating/i,
    );
  });

  it("keeps provisioning workspaces in the running group", () => {
    const creating: WorkspacesSidebarWorkspace = {
      id: "workspace-1",
      name: "New workspace",
      isCreating: true,
      isRunning: false,
    };

    const groups = categorizeWorkspaces([creating]);

    expect(groups.runningWorkspaces).toEqual([creating]);
    expect(groups.idleWorkspaces).toEqual([]);
  });
});
