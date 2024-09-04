import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/svelte";
import { DashboardNav } from "..";
import { mdiPlusBoxOutline, mdiSwapHorizontal, mdiTimerSand } from "@mdi/js";

describe("DashboardNav", () => {
  const baseProps = {
    items: [
      {
        href: "#",
        id: "item-1",
        label: "Something",
      },
      {
        href: "#",
        icons: [],
        id: "item-2",
        label: "Send",
      },
      {
        href: "#",
        icons: [{ path: mdiTimerSand }],
        id: "item-3",
        label: "Receive",
      },
      {
        href: "#",
        icons: [
          { path: mdiTimerSand },
          { path: mdiSwapHorizontal },
          { path: mdiPlusBoxOutline },
        ],
        id: "item-4",
        label: "Stake",
      },
    ],
  };
  const baseOptions = {
    props: baseProps,
    target: document.body,
  };

  afterEach(cleanup);

  it("renders the DashboardNav component", () => {
    const { container } = render(DashboardNav, baseOptions);

    expect(container.firstChild).toMatchSnapshot();
  });

  it("should pass additional class names and attributes to the rendered element", async () => {
    const props = {
      ...baseProps,
      className: "foo bar",
    };
    const { container, rerender } = render(DashboardNav, {
      ...baseOptions,
      props,
    });

    expect(container.firstChild).toHaveClass("foo bar");

    await rerender({
      ...props,
      className: "qux",
    });

    expect(container.firstChild).toHaveClass("qux");
  });

  it("should not display icons if the item's icons array is undefined", () => {
    const { container } = render(DashboardNav, baseProps);

    expect(
      container.querySelector(
        ".dashboard-nav-list > li:nth-child(1) > .dashboard-nav-list__item > .dashboard-nav-item-icons"
      )
    ).toBeNull();

    expect(container.firstChild).toMatchSnapshot();
  });

  it("should not display icons if the item's icon array is empty", () => {
    const { container } = render(DashboardNav, baseProps);

    expect(
      container.querySelector(
        ".dashboard-nav-list > li:nth-child(2) > .dashboard-nav-list__item > .dashboard-nav-item-icons"
      )
    ).toBeNull();

    expect(container.firstChild).toMatchSnapshot();
  });

  it("should display icons if the item's icon array is present", () => {
    const { container } = render(DashboardNav, baseProps);

    expect(
      container.querySelector(
        ".dashboard-nav-list > li:nth-child(3) > .dashboard-nav-list__item > .dashboard-nav-item-icons"
      )
    ).toBeTruthy();

    expect(container.firstChild).toMatchSnapshot();
  });
});