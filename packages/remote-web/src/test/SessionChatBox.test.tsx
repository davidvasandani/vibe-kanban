import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {
  SessionChatBox,
  DEFAULT_CONTINUE_PROMPT,
  type ExecutionStatus,
} from "@vibe/ui/components/SessionChatBox";

function renderChatBox(overrides: {
  status?: ExecutionStatus;
  value?: string;
  isNewSessionMode?: boolean;
}) {
  const onSend = vi.fn();
  render(
    <SessionChatBox
      status={overrides.status ?? "idle"}
      editor={{ value: overrides.value ?? "", onChange: vi.fn() }}
      renderEditor={() => <div data-testid="editor" />}
      actions={{
        onSend,
        onQueue: vi.fn(),
        onCancelQueue: vi.fn(),
        onStop: vi.fn(),
        onPasteFiles: vi.fn(),
      }}
      session={{
        sessions: [],
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
    const { onSend } = renderChatBox({ value: "" });

    const btn = sendButton();
    expect(btn).toBeEnabled();

    await userEvent.click(btn);
    expect(onSend).toHaveBeenCalledTimes(1);
  });

  it("keeps Send disabled with an empty editor in new-session mode", () => {
    renderChatBox({ value: "", isNewSessionMode: true });
    expect(sendButton()).toBeDisabled();
  });

  it("keeps Send enabled when the editor has content", () => {
    renderChatBox({ value: "do the thing" });
    expect(sendButton()).toBeEnabled();
  });

  it("advertises the default continue prompt in the placeholder", () => {
    // The placeholder must stay in sync with the prompt that gets sent.
    expect(DEFAULT_CONTINUE_PROMPT).toBe("Continue working on this task");
  });
});
