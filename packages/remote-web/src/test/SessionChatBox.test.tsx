import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {
  SessionChatBox,
  DEFAULT_CONTINUE_PROMPT,
  resolveSessionSendMessage,
  type ExecutionStatus,
} from "@vibe/ui/components/SessionChatBox";

function renderChatBox(overrides: {
  status?: ExecutionStatus;
  value?: string;
  isNewSessionMode?: boolean;
  selectedSessionId?: string;
  isDraftLoading?: boolean;
  issueSimpleId?: string;
}) {
  const onSend = vi.fn();
  const hasSession = "selectedSessionId" in overrides;
  render(
    <SessionChatBox
      status={overrides.status ?? "idle"}
      issueSimpleId={overrides.issueSimpleId}
      editor={{ value: overrides.value ?? "", onChange: vi.fn() }}
      isDraftLoading={overrides.isDraftLoading ?? false}
      renderEditor={() => <div data-testid="editor" />}
      actions={{
        onSend,
        onQueue: vi.fn(),
        onCancelQueue: vi.fn(),
        onStop: vi.fn(),
        onPasteFiles: vi.fn(),
      }}
      session={{
        sessions: hasSession
          ? [{ id: overrides.selectedSessionId!, created_at: "2026-01-01" }]
          : [],
        selectedSessionId: overrides.selectedSessionId,
        onSelectSession: vi.fn(),
        isNewSessionMode: overrides.isNewSessionMode ?? false,
      }}
    />,
  );
  return { onSend };
}

// Without an i18n provider, t() returns the key, so match the key or the
// translated label.
const sendButton = () => screen.getByRole("button", { name: /(^|\.)send$/i });

describe("SessionChatBox – sending the prefilled (placeholder) prompt", () => {
  it("enables Continue with an empty editor in an existing session", async () => {
    const { onSend } = renderChatBox({ value: "", selectedSessionId: "s1" });

    const btn = screen.getByRole("button", { name: "Continue" });
    expect(btn).toBeEnabled();

    await userEvent.click(btn);
    expect(onSend).toHaveBeenCalledTimes(1);
    expect(onSend).toHaveBeenCalledWith(DEFAULT_CONTINUE_PROMPT);
  });

  it("keeps Send disabled with an empty editor in new-session mode", () => {
    renderChatBox({ value: "", isNewSessionMode: true });
    expect(sendButton()).toBeDisabled();
  });

  it("keeps Send disabled in placeholder mode (no session selected)", () => {
    renderChatBox({ value: "" });
    expect(sendButton()).toBeDisabled();
  });

  it("keeps prompt actions disabled while a persisted draft is still loading", () => {
    // Enabling it here would render a clickable button that no-ops, because
    // the resolver refuses to substitute the prompt until the draft loads.
    renderChatBox({ value: "", selectedSessionId: "s1", isDraftLoading: true });
    for (const name of ["Continue", "What’s left", "Status"]) {
      expect(screen.getByRole("button", { name })).toBeDisabled();
    }
  });

  it("keeps Send enabled when the editor has content", () => {
    renderChatBox({ value: "do the thing", selectedSessionId: "s1" });
    expect(sendButton()).toBeEnabled();
  });
});

describe("empty composer prompt actions", () => {
  it.each([
    ["Continue", "Continue working on this task"],
    ["What’s left", "What’s left on VK-123"],
    ["Status", "Status"],
  ])("sends the exact %s prompt", async (label, prompt) => {
    const { onSend } = renderChatBox({
      value: "  ",
      selectedSessionId: "s1",
      issueSimpleId: "VK-123",
    });
    await userEvent.click(screen.getByRole("button", { name: label }));
    expect(onSend).toHaveBeenCalledTimes(1);
    expect(onSend).toHaveBeenCalledWith(prompt);
    expect(
      screen.queryByRole("button", { name: /(^|\.)send$/i }),
    ).not.toBeInTheDocument();
    expect(
      screen
        .getAllByRole("button")
        .map((button) => button.textContent)
        .filter((label) =>
          ["Continue", "What’s left", "Status"].includes(label ?? ""),
        ),
    ).toEqual(["Continue", "What’s left", "Status"]);
  });

  it("disables only the issue-specific prompt when identity is unavailable", async () => {
    const { onSend } = renderChatBox({ selectedSessionId: "s1" });
    expect(screen.getByRole("button", { name: "Continue" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Status" })).toBeEnabled();
    const remaining = screen.getByRole("button", { name: "What’s left" });
    expect(remaining).toBeDisabled();
    await userEvent.click(remaining);
    expect(onSend).not.toHaveBeenCalled();
  });

  it.each([
    "running",
    "queued",
    "sending",
    "stopping",
    "queue-loading",
  ] as const)("preserves %s controls", (status) => {
    renderChatBox({ selectedSessionId: "s1", status });
    expect(
      screen.queryByRole("button", { name: "Continue" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "What’s left" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Status" }),
    ).not.toBeInTheDocument();
  });

  it("sends typed text through the ordinary callback", async () => {
    const { onSend } = renderChatBox({
      value: "hello",
      selectedSessionId: "s1",
    });
    await userEvent.click(sendButton());
    expect(onSend).toHaveBeenCalledTimes(1);
    expect(onSend).toHaveBeenCalledWith();
    expect(
      screen.queryByRole("button", { name: "Continue" }),
    ).not.toBeInTheDocument();
  });
});

describe("resolveSessionSendMessage", () => {
  const base = {
    message: "",
    hasReviewComments: false,
    isNewSessionMode: false,
    isDraftLoaded: true,
  };

  it("substitutes the default continue prompt for an empty message in an existing session", () => {
    expect(resolveSessionSendMessage(base)).toBe(DEFAULT_CONTINUE_PROMPT);
    expect(resolveSessionSendMessage({ ...base, message: "   " })).toBe(
      DEFAULT_CONTINUE_PROMPT,
    );
  });

  it("keeps a typed message as-is", () => {
    expect(resolveSessionSendMessage({ ...base, message: "fix the bug" })).toBe(
      "fix the bug",
    );
  });

  it("does not substitute when review comments carry the content", () => {
    expect(
      resolveSessionSendMessage({ ...base, hasReviewComments: true }),
    ).toBe("");
  });

  it("does not substitute in new-session mode", () => {
    expect(resolveSessionSendMessage({ ...base, isNewSessionMode: true })).toBe(
      "",
    );
  });

  it("does not substitute while a persisted draft is still loading", () => {
    expect(resolveSessionSendMessage({ ...base, isDraftLoaded: false })).toBe(
      "",
    );
  });

  it("advertises the same prompt the placeholder shows", () => {
    // The placeholder must stay in sync with the prompt that gets sent.
    expect(DEFAULT_CONTINUE_PROMPT).toBe("Continue working on this task");
  });
});
