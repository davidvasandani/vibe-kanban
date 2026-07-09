import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AppBarOrgTile } from "@vibe/ui/components/AppBarOrgTile";

const orgs = [
  { id: "org-1", name: "Acme" },
  { id: "org-2", name: "Globex" },
  { id: "org-3", name: "Initech" },
];

describe("AppBarOrgTile", () => {
  it("renders nothing when there are no organizations", () => {
    const { container } = render(
      <AppBarOrgTile
        organizations={[]}
        selectedOrgId={null}
        onSelect={() => {}}
      />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("renders a single static tile with no expand toggle for one org", () => {
    render(
      <AppBarOrgTile
        organizations={[orgs[0]]}
        selectedOrgId="org-1"
        onSelect={() => {}}
      />,
    );
    expect(screen.getByRole("button", { name: "Acme" })).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /show organizations/i }),
    ).not.toBeInTheDocument();
  });

  it("shows only the active tile plus an expand toggle when collapsed", () => {
    render(
      <AppBarOrgTile
        organizations={orgs}
        selectedOrgId="org-2"
        onSelect={() => {}}
        expanded={false}
        onToggleExpanded={() => {}}
      />,
    );
    // Active org is shown as a tile.
    expect(screen.getByRole("button", { name: "Globex" })).toBeInTheDocument();
    // Other orgs are hidden while collapsed.
    expect(
      screen.queryByRole("button", { name: "Acme" }),
    ).not.toBeInTheDocument();
    // Expand toggle is present.
    expect(
      screen.getByRole("button", { name: /show organizations/i }),
    ).toBeInTheDocument();
  });

  it("calls onToggleExpanded when the collapsed control is clicked", async () => {
    const user = userEvent.setup();
    const onToggleExpanded = vi.fn();
    render(
      <AppBarOrgTile
        organizations={orgs}
        selectedOrgId="org-1"
        onSelect={() => {}}
        expanded={false}
        onToggleExpanded={onToggleExpanded}
      />,
    );
    await user.click(
      screen.getByRole("button", { name: /show organizations/i }),
    );
    expect(onToggleExpanded).toHaveBeenCalledTimes(1);
  });

  it("renders a tile per org when expanded and calls onSelect with the chosen id", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    render(
      <AppBarOrgTile
        organizations={orgs}
        selectedOrgId="org-1"
        onSelect={onSelect}
        expanded={true}
        onToggleExpanded={() => {}}
      />,
    );

    expect(screen.getByRole("button", { name: "Acme" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Globex" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Initech" })).toBeInTheDocument();
    // Collapse toggle present when expanded.
    expect(
      screen.getByRole("button", { name: /hide organizations/i }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Globex" }));
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith("org-2");
  });

  it("stays functional when uncontrolled (no onToggleExpanded): expands and selects", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    render(
      <AppBarOrgTile
        organizations={orgs}
        selectedOrgId="org-1"
        onSelect={onSelect}
      />,
    );

    // Collapsed: only the active tile + expand toggle; others hidden.
    expect(
      screen.queryByRole("button", { name: "Globex" }),
    ).not.toBeInTheDocument();

    // Clicking the toggle expands via internal state.
    await user.click(
      screen.getByRole("button", { name: /show organizations/i }),
    );
    expect(screen.getByRole("button", { name: "Acme" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Globex" })).toBeInTheDocument();

    // Switching still works without a controlled handler.
    await user.click(screen.getByRole("button", { name: "Globex" }));
    expect(onSelect).toHaveBeenCalledWith("org-2");
  });
});
