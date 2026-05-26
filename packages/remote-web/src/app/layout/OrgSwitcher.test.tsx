import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { OrgSwitcher } from "./OrgSwitcher";

const orgs = [
  { id: "org-1", name: "Acme" },
  { id: "org-2", name: "Globex" },
  { id: "org-3", name: "Initech" },
];

describe("OrgSwitcher", () => {
  it("renders nothing when there is only one organization", () => {
    const { container } = render(
      <OrgSwitcher
        organizations={[orgs[0]]}
        selectedOrgId="org-1"
        onSelect={() => {}}
      />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("renders nothing when there are no organizations", () => {
    const { container } = render(
      <OrgSwitcher
        organizations={[]}
        selectedOrgId={null}
        onSelect={() => {}}
      />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("renders a trigger when there are multiple organizations", () => {
    render(
      <OrgSwitcher
        organizations={orgs}
        selectedOrgId="org-1"
        onSelect={() => {}}
      />,
    );
    expect(
      screen.getByRole("button", { name: /switch organization/i }),
    ).toBeInTheDocument();
  });

  it("lists all organizations and calls onSelect with the chosen id", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();

    render(
      <OrgSwitcher
        organizations={orgs}
        selectedOrgId="org-1"
        onSelect={onSelect}
      />,
    );

    await user.click(
      screen.getByRole("button", { name: /switch organization/i }),
    );

    const items = await screen.findAllByRole("menuitem");
    expect(items.map((el) => el.textContent)).toEqual([
      "Acme",
      "Globex",
      "Initech",
    ]);

    await user.click(screen.getByRole("menuitem", { name: "Globex" }));
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith("org-2");
  });
});
