import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderWithProviders } from "@/test/render";

import { Sidebar } from "./sidebar";

vi.mock("@tanstack/react-router", () => ({
  Link: ({
    to,
    children,
    ...props
  }: {
    to: string;
    children: React.ReactNode;
  } & React.AnchorHTMLAttributes<HTMLAnchorElement>) => (
    <a href={to} {...props}>
      {children}
    </a>
  ),
}));

describe("Sidebar", () => {
  it("renders a fixed dock with active item and footer status", () => {
    const { container } = renderWithProviders(
      <Sidebar
        activeRoute="overview"
        health={{ status: "ok", version: "1.2.3" }}
      />,
    );

    const aside = container.querySelector("aside");
    expect(aside).toHaveClass("w-[76px]");
    expect(aside).not.toHaveTextContent("展开");
    expect(container.querySelector(".bg-gradient-to-br.from-blue-500.to-blue-700")).toBeTruthy();
    expect(screen.getByRole("link", { name: /cliswitch/i })).toHaveAttribute("href", "/");

    const activeLink = screen.getByRole("link", { name: /概览|overview/i });
    expect(activeLink).toHaveAttribute("href", "/");
    expect(activeLink).toHaveAttribute("aria-current", "page");
    expect(activeLink.className).toContain("before:w-[3px]");
    expect(activeLink.className).toContain("bg-blue-50");

    const nav = container.querySelector("nav");
    expect(nav?.querySelectorAll("a")).toHaveLength(7);
    expect(screen.getByText(/运行中|running/i)).toBeInTheDocument();
    expect(screen.getByText("v1.2.3")).toBeInTheDocument();
    expect(container.querySelector(".animate-pulse-dot.bg-emerald-500")).toBeTruthy();
  });
});
