import { createRef } from "react";
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import {
  KanbanIssuePanel,
  type IssueFormData,
} from "@vibe/ui/components/KanbanIssuePanel";

function renderPanel(overrides: {
  mode: "create" | "edit";
  issueId?: string | null;
  jiraLink?: { issueKey: string; url: string; active: boolean } | null;
}) {
  const formData: IssueFormData = {
    title: "Test issue",
    description: "Some description",
    statusId: "status-1",
    priority: null,
    assigneeIds: [],
    tagIds: [],
    createDraftWorkspace: false,
  };

  return render(
    <KanbanIssuePanel
      mode={overrides.mode}
      displayId="ISS-1"
      formData={formData}
      onFormChange={vi.fn()}
      statuses={[{ id: "status-1", name: "Todo", color: "#888888" }]}
      tags={[]}
      issueId={overrides.issueId}
      jiraLink={overrides.jiraLink}
      onClose={vi.fn()}
      onSubmit={vi.fn()}
      titleInputRef={createRef<HTMLTextAreaElement>()}
      renderDescriptionEditor={() => <div data-testid="description-editor" />}
      renderWorkspacesSection={() => <div data-testid="workspaces-section" />}
      renderCommentsSection={() => <div data-testid="comments-section" />}
    />,
  );
}

describe("KanbanIssuePanel – section order", () => {
  it("renders the workspaces section above the title and description in edit mode", () => {
    renderPanel({ mode: "edit", issueId: "issue-1" });

    const workspaces = screen.getByTestId("workspaces-section");
    const title = screen.getByRole("textbox", { name: /issue title/i });
    const description = screen.getByTestId("description-editor");
    const comments = screen.getByTestId("comments-section");

    // Node.DOCUMENT_POSITION_FOLLOWING (4): the argument comes after the node.
    expect(
      workspaces.compareDocumentPosition(title) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      workspaces.compareDocumentPosition(description) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    // Trailing sections stay below the description.
    expect(
      description.compareDocumentPosition(comments) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it("does not render the workspaces section in create mode", () => {
    renderPanel({ mode: "create", issueId: null });

    expect(screen.queryByTestId("workspaces-section")).not.toBeInTheDocument();
  });
});

describe("KanbanIssuePanel – Jira source link", () => {
  it("renders a link to the source Jira ticket when the issue has a link", () => {
    renderPanel({
      mode: "edit",
      issueId: "issue-1",
      jiraLink: {
        issueKey: "ABC-123",
        url: "https://example.atlassian.net/browse/ABC-123",
        active: true,
      },
    });

    const link = screen.getByRole("link", { name: /ABC-123/i });
    expect(link).toHaveAttribute(
      "href",
      "https://example.atlassian.net/browse/ABC-123",
    );
    expect(link).toHaveAttribute("target", "_blank");
  });

  it("does not render a Jira link when the issue has none", () => {
    renderPanel({ mode: "edit", issueId: "issue-1" });

    expect(screen.queryByRole("link", { name: /ABC-123/i })).toBeNull();
  });

  it("does not render a Jira link in create mode", () => {
    renderPanel({
      mode: "create",
      issueId: null,
      jiraLink: {
        issueKey: "ABC-123",
        url: "https://example.atlassian.net/browse/ABC-123",
        active: true,
      },
    });

    expect(screen.queryByRole("link", { name: /ABC-123/i })).toBeNull();
  });
});
