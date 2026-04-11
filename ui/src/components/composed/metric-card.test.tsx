import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderWithProviders } from "@/test/render";

import { MetricCard } from "./metric-card";

describe("MetricCard", () => {
  it("renders label, value, and left color bar", () => {
    const { container } = renderWithProviders(
      <MetricCard label="Requests" value="8,359" barColor="bg-primary" />,
    );

    expect(screen.getByText("Requests")).toHaveClass(
      "text-[11px]",
      "font-semibold",
      "uppercase",
      "tracking-wider",
      "text-muted-foreground",
    );
    expect(screen.getByText("8,359")).toHaveClass(
      "text-xl",
      "font-extrabold",
      "tracking-tight",
    );
    expect(
      container.querySelector(
        ".absolute.inset-y-3.left-0.w-\\[3px\\].bg-primary",
      ),
    ).toBeTruthy();
    expect(container.querySelector("svg")).toBeNull();
  });

  it("shows skeleton state without the value", () => {
    const { container } = renderWithProviders(
      <MetricCard label="Spend" value="$12.34" barColor="bg-success" loading />,
    );

    expect(screen.queryByText("$12.34")).toBeNull();
    expect(container.querySelector(".animate-shimmer")).toBeTruthy();
  });
});
