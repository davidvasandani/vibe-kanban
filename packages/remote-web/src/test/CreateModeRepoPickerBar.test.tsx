import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { CreateModeRepoPickerBar } from "@/shared/components/CreateModeRepoPickerBar";

// The repository step is the first half of the new-workspace config, and its
// Continue action commits it. It must obey the same contract as the prompt
// step's Create action: the repository list yields and scrolls, the controls
// row does not. Before this, the whole picker sat inside one scroller and
// Continue scrolled out of view with the list.
//
// JSDOM computes no layout, so these assert the structural contract. Browser
// measurement of the same markup put Continue 30px below the host before the
// fix and 8px inside it after.

// The picker reaches SettingsDialog, which pulls in a Vite virtual module this
// lane does not provide. Stub the dialog entrypoints so the test exercises the
// picker's layout rather than the settings tree.
vi.mock("@/shared/dialogs/settings/SettingsDialog", () => ({
  SettingsDialog: { show: vi.fn() },
}));
vi.mock("@/shared/dialogs/shared/FolderPickerDialog", () => ({
  FolderPickerDialog: { show: vi.fn() },
}));
vi.mock("@/shared/dialogs/command-bar/SelectionDialog", () => ({
  SelectionDialog: { show: vi.fn() },
}));

vi.mock("@/features/create-mode/model/useCreateMode", () => ({
  useCreateMode: () => ({
    repos: [
      { id: "r1", name: "alpha", display_name: null, setup_script: "x" },
      { id: "r2", name: "beta", display_name: null, setup_script: "x" },
    ],
    targetBranches: { r1: "main", r2: "main" },
    addRepo: vi.fn(),
    removeRepo: vi.fn(),
    setTargetBranch: vi.fn(),
  }),
}));

function renderBar() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const { container } = render(
    <QueryClientProvider client={client}>
      <CreateModeRepoPickerBar onContinueToPrompt={vi.fn()} />
    </QueryClientProvider>,
  );
  return container;
}

describe("CreateModeRepoPickerBar – Continue stays inside the host's height", () => {
  it("keeps every row that is not the list out of the shrink path", () => {
    const container = renderBar();

    // Siblings of the scroller must not absorb deficit; only the list yields.
    const scroller = container.querySelector(".overflow-y-auto")!;
    for (const sibling of Array.from(scroller.parentElement!.children)) {
      if (sibling === scroller) continue;
      expect(sibling).toHaveClass("shrink-0");
    }
  });

  it("scrolls the repository list and not the whole picker", () => {
    const container = renderBar();

    const scrollers = container.querySelectorAll(".overflow-y-auto");
    expect(scrollers.length).toBe(1);

    const list = scrollers[0]!;
    // A floor, not a zero minimum: the list is the terminal shrink target, and
    // a list collapsed to nothing hides every selected repository while
    // Continue stays enabled.
    expect(list).toHaveClass("min-h-[3rem]");
    // The scroller holds the repo rows...
    expect(list.querySelector(".rounded-sm.border")).not.toBeNull();
    // ...and not the action that commits the step.
    const cont = screen.getByRole("button", { name: /continue/i });
    expect(list).not.toContainElement(cont);
  });

  it("pins the controls row that carries Continue", () => {
    renderBar();

    const row = screen.getByRole("button", { name: /continue/i }).parentElement!
      .parentElement!;
    expect(row).toHaveClass("shrink-0");
  });

  it("makes the picker shrinkable so the list can absorb the deficit", () => {
    const container = renderBar();

    const root = container.firstElementChild!;
    expect(root).toHaveClass("flex", "min-h-0", "flex-col");
    const body = root.querySelector(".px-plusfifty")!;
    expect(body).toHaveClass("flex", "min-h-0", "flex-col");
  });
});
