import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderWithProviders } from "@/test/render";

import { PageHeader } from "./PageHeader";

describe("PageHeader", () => {
  it("renders v2 header layout with title and actions", () => {
    const { container } = renderWithProviders(
      <PageHeader
        title="Channels"
        actions={<button type="button">New</button>}
      />,
    );

    const header = container.querySelector("header");
    expect(header).toHaveClass("h-[55px]", "shrink-0", "items-center", "justify-between", "px-5");
    expect(screen.getByText("Channels")).toHaveClass("text-base", "font-bold");
    expect(screen.getByRole("button", { name: "New" })).toBeInTheDocument();
  });

  it("does not render the legacy invisible description placeholder", () => {
    const { container } = renderWithProviders(<PageHeader title="Logs" />);

    expect(screen.getByText("Logs")).toBeInTheDocument();
    expect(container.querySelector("p")).toBeNull();
  });
});
