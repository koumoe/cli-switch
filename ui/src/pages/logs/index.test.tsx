import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { NuqsTestingAdapter } from "nuqs/adapters/testing";
import { describe, expect, it, vi } from "vitest";

import { usageList } from "@/api";
import { renderWithProviders } from "@/test/render";

import { LogsPage } from "./index";

vi.mock("@/api", () => ({
  listChannels: vi.fn(async () => [
    {
      id: "channel-1",
      name: "Codex",
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

describe("LogsPage", () => {
  it("shows the account and channel names on separate lines", async () => {
    renderWithProviders(
      <NuqsTestingAdapter>
        <LogsPage />
      </NuqsTestingAdapter>,
    );

    const accountName = await screen.findByText("Hoxkai");
    const row = accountName.closest("tr");

    expect(row).not.toBeNull();
    const channelName = within(accountName.parentElement!).getByText("Codex");

    expect(accountName.parentElement).toBe(channelName.parentElement);
    expect(accountName.parentElement).toHaveClass("flex-col");
    expect(screen.getByRole("columnheader", { name: "渠道" })).toHaveClass(
      "w-[12%]",
    );
  });

  it("shows account and channel as separate detail fields", async () => {
    const user = userEvent.setup();

    renderWithProviders(
      <NuqsTestingAdapter>
        <LogsPage />
      </NuqsTestingAdapter>,
    );

    const accountName = await screen.findByText("Hoxkai");
    const row = accountName.closest("tr");

    expect(row).not.toBeNull();
    await user.click(within(row!).getByRole("button", { name: "详情" }));

    const dialog = await screen.findByRole("dialog");
    const accountLabel = within(dialog).getByText("账号");
    const channelLabel = within(dialog).getByText("渠道");

    expect(accountLabel.nextElementSibling).toHaveTextContent("Hoxkai");
    expect(channelLabel.nextElementSibling).toHaveTextContent("Codex");
    expect(channelLabel.nextElementSibling).not.toHaveTextContent("Hoxkai");
  });

  it("labels channel options with account and channel names", async () => {
    const user = userEvent.setup();

    renderWithProviders(
      <NuqsTestingAdapter>
        <LogsPage />
      </NuqsTestingAdapter>,
    );

    await screen.findByText("Hoxkai");
    const channelSelect = screen.getAllByRole("combobox")[1];

    await user.click(channelSelect);

    expect(
      await screen.findByRole("option", { name: "Hoxkai · Codex" }),
    ).toBeInTheDocument();
  });

  it("keeps a compatible protocol and channel selection while channels load", async () => {
    renderWithProviders(
      <NuqsTestingAdapter
        hasMemory
        searchParams="?protocol=openai&channel=channel-1"
      >
        <LogsPage />
      </NuqsTestingAdapter>,
    );

    await screen.findByText("Hoxkai");
    const [protocolSelect, channelSelect] = screen.getAllByRole("combobox");

    await waitFor(() => {
      expect(protocolSelect).toHaveTextContent("Codex");
      expect(channelSelect).toHaveTextContent("Hoxkai · Codex");
    });
  });

  it("combines protocol, channel, and status filters", async () => {
    const user = userEvent.setup();
    const usageListMock = vi.mocked(usageList);

    renderWithProviders(
      <NuqsTestingAdapter hasMemory>
        <LogsPage />
      </NuqsTestingAdapter>,
    );

    await screen.findByText("Hoxkai");
    const [protocolSelect, channelSelect, statusSelect] =
      screen.getAllByRole("combobox");

    await user.click(channelSelect);
    await user.click(
      await screen.findByRole("option", { name: "Hoxkai · Codex" }),
    );

    expect(protocolSelect).toHaveTextContent("Codex");
    expect(channelSelect).toHaveTextContent("Hoxkai · Codex");

    await user.click(statusSelect);
    await user.click(await screen.findByRole("option", { name: "失败" }));

    usageListMock.mockClear();
    await user.click(screen.getByRole("button", { name: "查询" }));

    await waitFor(() => {
      expect(usageListMock).toHaveBeenCalledWith(
        expect.objectContaining({
          protocol: "openai",
          channel_id: "channel-1",
          success: false,
        }),
      );
    });
  });

  it("preserves protocol and channel when status changes after selecting protocol first", async () => {
    const user = userEvent.setup();
    const onUrlUpdate = vi.fn();

    renderWithProviders(
      <NuqsTestingAdapter onUrlUpdate={onUrlUpdate}>
        <LogsPage />
      </NuqsTestingAdapter>,
    );

    await screen.findByText("Hoxkai");
    const [protocolSelect, channelSelect, statusSelect] =
      screen.getAllByRole("combobox");

    await user.click(protocolSelect);
    await user.click(await screen.findByRole("option", { name: "Codex" }));
    await waitFor(() => expect(protocolSelect).toHaveTextContent("Codex"));

    await user.click(channelSelect);
    await user.click(
      await screen.findByRole("option", { name: "Hoxkai · Codex" }),
    );
    await waitFor(() => {
      expect(protocolSelect).toHaveTextContent("Codex");
      expect(channelSelect).toHaveTextContent("Hoxkai · Codex");
    });

    await user.click(statusSelect);
    await user.click(await screen.findByRole("option", { name: "失败" }));
    await waitFor(() => {
      const lastUpdate = onUrlUpdate.mock.calls.at(-1)?.[0];

      expect(lastUpdate?.searchParams.get("protocol")).toBe("openai");
      expect(lastUpdate?.searchParams.get("channel")).toBe("channel-1");
      expect(lastUpdate?.searchParams.get("status")).toBe("failed");
    });
  });
});
