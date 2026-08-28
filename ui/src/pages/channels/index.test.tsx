import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderWithProviders } from "@/test/render";

import { ChannelsPage } from "./index";

vi.mock("@/api", () => ({
  createChannel: vi.fn(),
  deleteChannel: vi.fn(),
  disableChannel: vi.fn(),
  enableChannel: vi.fn(),
  getUsdCnyExchangeRate: vi.fn(async () => ({
    base_currency: "USD",
    quote_currency: "CNY",
    rate: 6.72,
    effective_date: "2026-08-28",
    source: "Frankfurter",
    fetched_at_ms: 1_777_000_000_000,
    stale: false,
  })),
  getSettings: vi.fn(async () => null),
  listChannels: vi.fn(async () => [
    {
      id: "channel-1",
      name: "Channel name",
      protocol: "openai",
      base_url: "https://api.example.com",
      auth_type: "managed_account",
      auth_ref: "",
      checkin_url: null,
      priority: 1,
      retry_times: 1,
      ignore_channel_protection: false,
      recharge_currency: "USD",
      real_multiplier: 1,
      enabled: true,
      auto_disabled_until_ms: 0,
      managed_by_remote: true,
      managed_remote_provider: "openai",
      managed_remote_account_id: "account-1",
      managed_remote_resource_id: "remote-1",
      managed_remote_resource_name: null,
      managed_remote_group_name: null,
      managed_remote_group_id: null,
      created_at_ms: 1,
      updated_at_ms: 1,
    },
  ]),
  listRemoteAccounts: vi.fn(async () => [
    {
      id: "account-1",
      name: "Configured account name",
      remote_username: "username-should-not-render",
    },
  ]),
  reorderChannels: vi.fn(),
  testChannel: vi.fn(),
  updateChannel: vi.fn(),
}));

describe("ChannelsPage account and channel names", () => {
  it("shows the configured account name separately from the channel name", async () => {
    renderWithProviders(<ChannelsPage />);

    expect(
      await screen.findByText("Configured account name"),
    ).toBeInTheDocument();
    expect(screen.getByText("Channel name")).toBeInTheDocument();
    expect(
      screen.getByRole("columnheader", { name: "账号" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("columnheader", { name: "名称" }),
    ).toBeInTheDocument();
    expect(screen.queryByText("username-should-not-render")).not.toBeInTheDocument();
  });
});
