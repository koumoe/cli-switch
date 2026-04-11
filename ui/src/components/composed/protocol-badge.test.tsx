import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderWithProviders } from "@/test/render";

import { ProtocolBadge } from "./protocol-badge";

describe("ProtocolBadge", () => {
  it("uses the shared neutral badge style and protocol icons", () => {
    const { container } = renderWithProviders(
      <ProtocolBadge protocol="openai">OpenAI</ProtocolBadge>,
    );

    const badge = screen.getByText("OpenAI");
    const icons = container.querySelectorAll("[data-slot='protocol-icon']");

    expect(badge).toHaveClass(
      "inline-flex",
      "items-center",
      "gap-1.5",
      "rounded-md",
      "px-2",
      "py-0.5",
      "text-[10px]",
      "font-medium",
    );
    expect(badge.className).toContain("border-border/80");
    expect(badge.className).toContain("bg-secondary/55");
    expect(badge.className).toContain("text-foreground");
    expect(icons).toHaveLength(2);
    expect(icons[0]).toHaveClass("h-3", "w-3", "dark:hidden");
    expect(icons[1]).toHaveClass("hidden", "dark:block");
    expect((icons[0] as HTMLImageElement).getAttribute("src")).toContain(
      "/assets/protocol-icons/light/openai",
    );
  });

  it("maps anthropic and gemini to their local icon assets", () => {
    const { container } = renderWithProviders(
      <div>
        <ProtocolBadge protocol="anthropic">Anthropic</ProtocolBadge>
        <ProtocolBadge protocol="gemini">Gemini</ProtocolBadge>
      </div>,
    );

    const icons = container.querySelectorAll("[data-slot='protocol-icon']");
    expect(screen.getByText("Anthropic").className).toContain(
      "bg-secondary/55",
    );
    expect(screen.getByText("Gemini").className).toContain("bg-secondary/55");
    expect(icons).toHaveLength(4);
    expect((icons[0] as HTMLImageElement).getAttribute("src")).toContain(
      "/assets/protocol-icons/light/claude",
    );
    expect((icons[2] as HTMLImageElement).getAttribute("src")).toContain(
      "/assets/protocol-icons/light/gemini",
    );
  });
});
