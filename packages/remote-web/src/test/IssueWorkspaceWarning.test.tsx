import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import type { PullRequest, Workspace } from "shared/remote-types";
import { PROJECT_WORKSPACES_SHAPE } from "shared/remote-types";
import type { SidebarWorkspace } from "@/shared/hooks/useWorkspaces";
import { getIssueWorkspaceAdvisory } from "@/shared/lib/issueWorkspaceAdvisory";
import { IssueWorkspaceWarningContainer } from "@/shared/components/IssueWorkspaceWarningContainer";
import { IssueWorkspaceWarning } from "@vibe/ui/components/IssueWorkspaceWarning";
import { IssueWorkspacesSection } from "@vibe/ui/components/IssueWorkspacesSection";
import type { WorkspaceWithStats } from "@vibe/ui/components/IssueWorkspaceCard";
import i18n from "@/i18n";

const state = vi.hoisted(() => ({
  workspaces: [] as Workspace[],
  prs: [] as PullRequest[],
  locals: [] as SidebarWorkspace[],
  navigate: vi.fn(),
}));
vi.mock("@/shared/integrations/electric/hooks", () => ({
  useShape: (shape: unknown) => ({
    data: shape === PROJECT_WORKSPACES_SHAPE ? state.workspaces : state.prs,
  }),
}));
vi.mock("@/shared/hooks/useWorkspaceContext", () => ({
  useWorkspaceContext: () => ({
    activeWorkspaces: state.locals,
    archivedWorkspaces: [],
  }),
}));
vi.mock("@/shared/hooks/auth/useAuth", () => ({
  useAuth: () => ({ userId: "owner" }),
}));
vi.mock("@/shared/hooks/useAppNavigation", () => ({
  useAppNavigation: () => ({ goToProjectIssueWorkspace: state.navigate }),
}));

function workspace(id: string, overrides: Partial<Workspace> = {}): Workspace {
  return {
    id,
    project_id: "project",
    issue_id: "issue",
    local_workspace_id: `local-${id}`,
    owner_user_id: "owner",
    name: `Workspace ${id}`,
    archived: false,
    files_changed: null,
    lines_added: null,
    lines_removed: null,
    created_at: "",
    updated_at: "",
    ...overrides,
  };
}
function local(
  id: string,
  overrides: Partial<SidebarWorkspace> = {},
): SidebarWorkspace {
  return {
    id: `local-${id}`,
    name: `Local ${id}`,
    branch: `vk/${id}`,
    createdAt: "",
    updatedAt: "",
    description: "",
    isRunning: true,
    ...overrides,
  };
}
function pr(id: string, status: PullRequest["status"] = "open"): PullRequest {
  return {
    id: `pr-${id}`,
    workspace_id: id,
    project_id: "project",
    issue_id: "issue",
    status,
    url: "https://example.com/pr/1",
    number: 1,
    merged_at: null,
    merge_commit_sha: null,
    target_branch_name: "main",
    created_at: "",
    updated_at: "",
  };
}
function project() {
  return getIssueWorkspaceAdvisory({
    projectId: "project",
    issueId: "issue",
    workspaces: state.workspaces,
    pullRequests: state.prs,
    localWorkspaces: state.locals,
    userId: "owner",
  });
}
beforeEach(async () => {
  await i18n.changeLanguage("en");
  state.workspaces = [];
  state.prs = [];
  state.locals = [];
  state.navigate.mockClear();
  localStorage.clear();
});

describe("issue workspace advisory", () => {
  it("matches issue and project identity and excludes archived records, regardless of titles", () => {
    state.workspaces = [
      workspace("a"),
      workspace("archived", { archived: true }),
      workspace("other", { issue_id: "another-issue", name: "Workspace a" }),
      workspace("other-project", { project_id: "another-project" }),
    ];
    expect(project().map((row) => row.id)).toEqual(["a"]);
  });
  it("joins branch/activity by local ID and PR evidence by remote workspace ID", () => {
    state.workspaces = [workspace("a"), workspace("b"), workspace("c")];
    state.locals = [
      local("a"),
      local("b", { isRunning: false, filesChanged: 2 }),
    ];
    state.prs = [pr("a"), pr("b", "merged"), pr("unrelated")];
    const rows = project();
    expect(rows[0]).toMatchObject({
      id: "a",
      branch: "vk/a",
      activity: "running",
      hasOpenPr: true,
    });
    expect(rows[1]).toMatchObject({
      id: "b",
      branch: "vk/b",
      activity: "idle",
      hasOpenPr: false,
      hasChanges: true,
    });
    expect(rows[2]).toMatchObject({
      id: "c",
      branch: null,
      activity: "unknown",
      hasOpenPr: false,
    });
  });
  it("warns about other owners without offering unauthorized navigation", () => {
    state.workspaces = [workspace("a", { owner_user_id: "someone-else" })];
    render(
      <IssueWorkspaceWarningContainer projectId="project" issueId="issue" />,
    );
    expect(screen.getByText("Workspace a")).toBeVisible();
    expect(screen.getByText("Branch unavailable")).toBeVisible();
    expect(
      screen.queryByRole("button", { name: "Open workspace" }),
    ).not.toBeInTheDocument();
  });
  it("keeps an owned sibling on another host visible without navigating to the current host", () => {
    state.workspaces = [workspace("elsewhere")];
    const { rerender } = render(
      <IssueWorkspaceWarningContainer projectId="project" issueId="issue" />,
    );
    expect(screen.getByText("Workspace elsewhere")).toBeVisible();
    expect(
      screen.queryByRole("button", { name: "Open workspace" }),
    ).not.toBeInTheDocument();
    state.locals = [local("elsewhere")];
    rerender(
      <IssueWorkspaceWarningContainer projectId="project" issueId="issue" />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Open workspace" }));
    expect(state.navigate).toHaveBeenCalledWith(
      "project",
      "issue",
      "local-elsewhere",
    );
  });
  it("shows no warning for zero active siblings", () => {
    state.workspaces = [workspace("a", { archived: true })];
    render(
      <IssueWorkspaceWarningContainer projectId="project" issueId="issue" />,
    );
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });
  it("dismisses, ignores enrichment/order changes, and reappears for a new sibling", () => {
    state.workspaces = [workspace("a"), workspace("b")];
    const { rerender } = render(
      <IssueWorkspaceWarningContainer projectId="project" issueId="issue" />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Dismiss warning" }));
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    state.prs = [pr("b")];
    state.workspaces.reverse();
    rerender(
      <IssueWorkspaceWarningContainer projectId="project" issueId="issue" />,
    );
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    state.workspaces.push(workspace("c"));
    rerender(
      <IssueWorkspaceWarningContainer projectId="project" issueId="issue" />,
    );
    expect(screen.getByRole("status")).toBeVisible();
    expect(screen.getAllByRole("listitem")).toHaveLength(3);
  });
  it("opens the matching workspace without requiring dismissal", () => {
    state.workspaces = [workspace("a"), workspace("b")];
    state.locals = [local("a"), local("b")];
    state.prs = [pr("a")];

    render(
      <IssueWorkspaceWarningContainer projectId="project" issueId="issue" />,
    );
    const rows = screen.getAllByRole("listitem");
    expect(within(rows[0]).getByText("Open pull request")).toBeVisible();
    expect(
      within(rows[1]).queryByText("Open pull request"),
    ).not.toBeInTheDocument();
    fireEvent.click(
      within(rows[1]).getByRole("button", { name: "Open workspace" }),
    );
    expect(state.navigate).toHaveBeenCalledWith("project", "issue", "local-b");
  });
  it("does not carry dismissal to a different issue", () => {
    state.workspaces = [workspace("a"), workspace("b", { issue_id: "second" })];
    const { rerender } = render(
      <IssueWorkspaceWarningContainer projectId="project" issueId="issue" />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Dismiss warning" }));
    rerender(
      <IssueWorkspaceWarningContainer projectId="project" issueId="second" />,
    );
    expect(screen.getByText("Workspace b")).toBeVisible();
  });
});

it("shows a plural active count on a collapsed issue section, excluding archives", () => {
  localStorage.setItem("vibe.ui.collapsible.kanban-issue-workspaces", "false");
  const workspaces = ["a", "b", "c"].map(
    (id): WorkspaceWithStats => ({
      id,
      localWorkspaceId: null,
      name: id,
      archived: id === "c",
      filesChanged: 0,
      linesAdded: 0,
      linesRemoved: 0,
      prs: [],
      owner: null,
      updatedAt: "",
      isOwnedByCurrentUser: false,
    }),
  );
  const { rerender } = render(
    <IssueWorkspacesSection workspaces={workspaces} />,
  );
  expect(screen.getByText("2 active workspaces")).toBeVisible();
  expect(screen.queryByText("a")).not.toBeInTheDocument();
  rerender(
    <IssueWorkspacesSection
      workspaces={workspaces.map((row) => ({
        ...row,
        archived: row.id !== "a",
      }))}
    />,
  );
  expect(screen.queryByText("2 active workspaces")).not.toBeInTheDocument();
});

// Regression: the create-workspace column is a gapped flex column, so the
// advisory must contribute no flex child at all when it has nothing to say.
// Wrapping it in a `shrink-0` div instead of passing the class onto its own
// root left an empty element behind and added a stray gap above the heading.
describe("IssueWorkspaceWarning – layout classes do not outlive the content", () => {
  it("renders nothing when there are no sibling workspaces, even with a className", () => {
    const { container } = render(
      <IssueWorkspaceWarning
        workspaces={[]}
        className="shrink-0"
        onDismiss={() => {}}
        onOpen={() => {}}
      />,
    );

    expect(container).toBeEmptyDOMElement();
  });

  it("applies a caller className to its own root when it does render", () => {
    const { container } = render(
      <IssueWorkspaceWarning
        workspaces={[
          {
            id: "w1",
            name: "existing",
            branch: "main",
            localWorkspaceId: "w1",
            activity: "idle",
            hasOpenPr: false,
            hasChanges: false,
          },
        ]}
        className="shrink-0"
        onDismiss={() => {}}
        onOpen={() => {}}
      />,
    );

    expect(container.firstElementChild).toHaveClass("shrink-0");
  });
});
