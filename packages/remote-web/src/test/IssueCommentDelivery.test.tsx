import { describe, it, expect, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import {
  IssueCommentConversationCard,
  IssueCommentDeliveryChip,
  IssueCommentIntentToggle,
} from "@vibe/ui/components/IssueCommentDelivery";

describe("IssueCommentIntentToggle", () => {
  it("makes note vs. reply an explicit choice and explains it", () => {
    const onChange = vi.fn();
    render(
      <IssueCommentIntentToggle
        value="reply"
        onChange={onChange}
        noteLabel="Note"
        replyLabel="Reply to agent"
        hint="Resumes AMT MCP, which is waiting for input."
      />,
    );
    const reply = screen.getByRole("radio", { name: "Reply to agent" });
    expect(reply.getAttribute("aria-checked")).toBe("true");
    expect(
      screen.getByText("Resumes AMT MCP, which is waiting for input."),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole("radio", { name: "Note" }));
    expect(onChange).toHaveBeenCalledWith("note");
  });

  it("disables reply with a reason when nothing is eligible", () => {
    render(
      <IssueCommentIntentToggle
        value="note"
        onChange={vi.fn()}
        noteLabel="Note"
        replyLabel="Reply to agent"
        replyDisabled
        replyDisabledReason="the linked workspace is archived"
      />,
    );
    const reply = screen.getByRole("radio", { name: "Reply to agent" });
    expect((reply as HTMLButtonElement).disabled).toBe(true);
    expect(reply.getAttribute("title")).toBe(
      "the linked workspace is archived",
    );
  });
});

describe("IssueCommentDeliveryChip", () => {
  it("shows the outcome, the verbatim diagnostic and its actions", () => {
    const onRetry = vi.fn();
    render(
      <IssueCommentDeliveryChip
        tone="warning"
        label="Delivery failed · the agent could not be started"
        detail={"could not start a turn:\nworker offline"}
        actions={[{ key: "retry", label: "Retry", onClick: onRetry }]}
      />,
    );
    const chip = screen.getByTestId("issue-comment-delivery");
    expect(chip.getAttribute("data-tone")).toBe("warning");
    expect(
      screen.getByText(
        (_, node) =>
          node?.textContent === "could not start a turn:\nworker offline",
      ),
    ).toBeTruthy();
    fireEvent.click(screen.getByText("Retry"));
    expect(onRetry).toHaveBeenCalledOnce();
  });
});

describe("IssueCommentConversationCard", () => {
  it("marks the entry as an issue comment with a route back", () => {
    const onOpen = vi.fn();
    render(
      <IssueCommentConversationCard
        indicatorLabel="Issue comment · VAS-859"
        authorName="Ada"
        timeLabel={<time dateTime="2026-10-19T04:45:00Z">04:45</time>}
        message={<p>Device address is 10.0.0.5</p>}
        notices={["Edited after delivery"]}
        openIssue={{ label: "Open on issue", onClick: onOpen }}
      />,
    );
    expect(screen.getByText("Issue comment · VAS-859")).toBeTruthy();
    expect(screen.getByText("Device address is 10.0.0.5")).toBeTruthy();
    expect(screen.getByText("Edited after delivery")).toBeTruthy();
    fireEvent.click(screen.getByText("Open on issue"));
    expect(onOpen).toHaveBeenCalledOnce();
  });
});
