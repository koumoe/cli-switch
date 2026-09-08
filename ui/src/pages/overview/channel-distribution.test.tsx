import { describe, expect, it } from "vitest";

import { renderWithProviders } from "@/test/render";
import type { Channel, ChannelStats } from "@/types/api";

import { ChannelDistribution } from "./channel-distribution";

const protocolLabel = (protocol: "openai" | "anthropic" | "gemini") => protocol;

describe("ChannelDistribution", () => {
  it("preserves equal channel labels with distinct accounts and unchanged usage shares", () => {
    const stats = [
      { channel_id: "a", name: "OpenAI 20X", protocol: "openai", success: 60 },
      { channel_id: "b", name: "OpenAI 20X", protocol: "openai", success: 40 },
    ] as ChannelStats[];
    const channels = new Map([
      ["a", { id: "a", name: "OpenAI 20X", managed_remote_account_id: "account-a" } as Channel],
      ["b", { id: "b", name: "OpenAI 20X", managed_remote_account_id: "account-b" } as Channel],
    ]);
    const { getByRole, rerender, queryByText, getByText } = renderWithProviders(
      <ChannelDistribution stats={stats} channelsById={channels}
        accountNames={{ "account-a": "Gmail", "account-b": "Icloud" }}
        protocolLabel={protocolLabel} view="percent" />,
    );
    expect(getByRole("progressbar", { name: "Gmail · OpenAI 20X" })).toHaveAttribute("aria-valuenow", "60");
    expect(getByRole("progressbar", { name: "Icloud · OpenAI 20X" })).toHaveAttribute("aria-valuenow", "40");
    rerender(<ChannelDistribution stats={stats} channelsById={channels}
      accountNames={{ "account-a": "Renamed", "account-b": "Icloud" }}
      protocolLabel={protocolLabel} view="percent" />);
    expect(queryByText("Gmail")).not.toBeInTheDocument();
    expect(getByText("Renamed")).toBeInTheDocument();
    expect(getByRole("progressbar", { name: "Renamed · OpenAI 20X" })).toHaveAttribute("aria-valuenow", "60");
  });

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
        channelsById={new Map()}
        accountNames={{}}
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
        channelsById={new Map()}
        accountNames={{}}
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
        channelsById={new Map()}
        accountNames={{}}
        protocolLabel={protocolLabel}
        view="usage"
      />,
    );

    expect(getByText("20")).toBeInTheDocument();
    expect(getByText("5")).toBeInTheDocument();
    expect(container.querySelector(".bg-primary")).toBeTruthy();
  });
});
