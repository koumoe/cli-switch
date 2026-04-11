import { describe, expect, it } from "vitest";

import { renderWithProviders } from "@/test/render";

import { ChannelDistribution } from "./channel-distribution";

const protocolLabel = (protocol: "openai" | "anthropic" | "gemini") => protocol;

describe("ChannelDistribution", () => {
  it("renders nothing when the total usage is zero", () => {
    const { container } = renderWithProviders(
      <ChannelDistribution
        stats={[
          {
            channel_id: "a",
            name: "Alpha",
            protocol: "openai",
            success: 0,
            requests: 0,
            failed: 0,
            estimated_cost_usd: null,
            avg_latency_ms: null,
            total_tokens: 0,
          },
        ]}
        protocolLabel={protocolLabel}
        view="percent"
      />,
    );

    expect(container.firstChild).toBeNull();
  });

  it("sorts rows by success and shows percent values", () => {
    const { container, getByText } = renderWithProviders(
      <ChannelDistribution
        stats={[
          {
            channel_id: "a",
            name: "Alpha",
            protocol: "openai",
            success: 20,
            requests: 20,
            failed: 0,
            estimated_cost_usd: null,
            avg_latency_ms: null,
            total_tokens: 0,
          },
          {
            channel_id: "b",
            name: "Beta",
            protocol: "anthropic",
            success: 60,
            requests: 60,
            failed: 0,
            estimated_cost_usd: null,
            avg_latency_ms: null,
            total_tokens: 0,
          },
          {
            channel_id: "c",
            name: "Gamma",
            protocol: "gemini",
            success: 20,
            requests: 20,
            failed: 0,
            estimated_cost_usd: null,
            avg_latency_ms: null,
            total_tokens: 0,
          },
        ]}
        protocolLabel={protocolLabel}
        view="percent"
      />,
    );

    const text = container.textContent ?? "";
    expect(text.indexOf("Beta")).toBeLessThan(text.indexOf("Alpha"));
    expect(text.indexOf("Alpha")).toBeLessThan(text.indexOf("Gamma"));
    expect(getByText("60%")).toBeInTheDocument();
    expect(container.querySelector(".bg-primary")).toBeTruthy();
  });

  it("shows usage counts in usage mode", () => {
    const { getByText, container } = renderWithProviders(
      <ChannelDistribution
        stats={[
          {
            channel_id: "a",
            name: "Alpha",
            protocol: "openai",
            success: 20,
            requests: 20,
            failed: 0,
            estimated_cost_usd: null,
            avg_latency_ms: null,
            total_tokens: 0,
          },
          {
            channel_id: "b",
            name: "Beta",
            protocol: "anthropic",
            success: 5,
            requests: 5,
            failed: 0,
            estimated_cost_usd: null,
            avg_latency_ms: null,
            total_tokens: 0,
          },
        ]}
        protocolLabel={protocolLabel}
        view="usage"
      />,
    );

    expect(getByText("20")).toBeInTheDocument();
    expect(getByText("5")).toBeInTheDocument();
    expect(container.querySelector(".bg-primary")).toBeTruthy();
  });
});
