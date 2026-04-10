import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderWithProviders } from "@/test/render";

import { ProtocolBadge } from "./protocol-badge";

describe("ProtocolBadge", () => {
  it("uses the shared base classes and openai palette", () => {
    renderWithProviders(<ProtocolBadge protocol="openai">OpenAI</ProtocolBadge>);

    const badge = screen.getByText("OpenAI");
    expect(badge).toHaveClass(
      "inline-flex",
      "items-center",
      "rounded-md",
      "px-2",
      "py-0.5",
      "text-[10px]",
      "font-bold",
      "tracking-wide",
      "bg-teal-50",
      "text-teal-600",
    );
    expect(badge.className).not.toContain("codex");
  });

  it("maps anthropic and gemini to orange and blue palettes", () => {
    renderWithProviders(
      <div>
        <ProtocolBadge protocol="anthropic">Anthropic</ProtocolBadge>
        <ProtocolBadge protocol="gemini">Gemini</ProtocolBadge>
      </div>,
    );

    expect(screen.getByText("Anthropic")).toHaveClass("bg-orange-50", "text-orange-600");
    expect(screen.getByText("Gemini")).toHaveClass("bg-blue-50", "text-blue-600");
  });
});
