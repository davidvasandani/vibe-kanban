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
}) {
  const onSend = vi.fn();
  const hasSession = "selectedSessionId" in overrides;
  render(
    <SessionChatBox
      status={overrides.status ?? "idle"}
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
  it("enables Send with an empty editor when continuing an existing session", async () => {
    const { onSend } = renderChatBox({ value: "", selectedSessionId: "s1" });

    const btn = sendButton();
    expect(btn).toBeEnabled();

    await userEvent.click(btn);
    expect(onSend).toHaveBeenCalledTimes(1);
  });

  it("keeps Send disabled with an empty editor in new-session mode", () => {
    renderChatBox({ value: "", isNewSessionMode: true });
    expect(sendButton()).toBeDisabled();
  });

  it("keeps Send disabled in placeholder mode (no session selected)", () => {
    renderChatBox({ value: "" });
    expect(sendButton()).toBeDisabled();
  });

  it("keeps Send disabled while a persisted draft is still loading", () => {
    // Enabling it here would render a clickable button that no-ops, because
    // the resolver refuses to substitute the prompt until the draft loads.
    renderChatBox({ value: "", selectedSessionId: "s1", isDraftLoading: true });
    expect(sendButton()).toBeDisabled();
  });

  it("keeps Send enabled when the editor has content", () => {
    renderChatBox({ value: "do the thing", selectedSessionId: "s1" });
    expect(sendButton()).toBeEnabled();
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
