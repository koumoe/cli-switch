import { screen, within } from "@testing-library/react";
import { NuqsTestingAdapter } from "nuqs/adapters/testing";
import { describe, expect, it, vi } from "vitest";

import { renderWithProviders } from "@/test/render";

import { LogsPage } from "./index";

vi.mock("@/api", () => ({
  listChannels: vi.fn(async () => [
    {
      id: "channel-1",
      name: "Codex Channel",
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
      name: "Hoxkai",
    },
  ]),
  usageList: vi.fn(async () => ({
    total: 1,
    items: [
      {
        id: "event-1",
        request_id: "request-1",
        ts_ms: 1_700_000_000_000,
        protocol: "openai",
        channel_id: "channel-1",
        model: "gpt-5.6-sol",
        success: true,
        http_status: 200,
        error_kind: null,
        error_detail: null,
        latency_ms: 1_000,
        ttft_ms: 500,
        prompt_tokens: 100,
        completion_tokens: 20,
        total_tokens: 120,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        estimated_cost_usd: "0.01",
      },
    ],
  })),
}));

describe("LogsPage channel display", () => {
  it("shows the account and channel names on separate lines", async () => {
    renderWithProviders(
      <NuqsTestingAdapter>
        <LogsPage />
      </NuqsTestingAdapter>,
    );

    const accountName = await screen.findByText("Hoxkai");
    const row = accountName.closest("tr");

    expect(row).not.toBeNull();
    const channelName = within(row!).getByText("Codex Channel");

    expect(accountName.parentElement).toBe(channelName.parentElement);
    expect(accountName.parentElement).toHaveClass("flex-col");
    expect(screen.getByRole("columnheader", { name: "渠道" })).toHaveClass(
      "w-[12%]",
    );
  });
});
