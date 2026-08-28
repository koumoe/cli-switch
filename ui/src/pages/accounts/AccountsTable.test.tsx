import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { renderWithProviders } from "@/test/render";
import type { OpenAiRemoteAccount } from "@/types/api";

import { AccountsTable } from "./AccountsTable";

const openAiAccount: OpenAiRemoteAccount = {
  id: "openai-account-1",
  name: "Personal OpenAI",
  provider: "openai",
  base_url: "https://chatgpt.com",
  api_url: null,
  user_id: "account-123",
  user_token_configured: true,
  reauth_required: false,
  page_checkin_url: null,
  checkin_mode: "disabled",
  auto_checkin_enabled: false,
  auto_checkin_time: "00:05:00",
  low_balance_alert_threshold: 0,
  recharge_currency: "USD",
  remote_username: "codex@example.com",
  remote_display_name: null,
  last_balance_amount: null,
  last_sync_error: null,
  last_synced_at_ms: null,
  low_balance_alert_notified: false,
  last_balance_alert_at_ms: null,
  sort_order: 0,
  created_at_ms: 1,
  updated_at_ms: 1,
  account_id: "account-123",
  plan_type: "plus",
  token_expires_at_ms: null,
  quota_windows: [
    {
      kind: "primary",
      limit_name: null,
      used_percent: 43,
      window_minutes: 10_080,
      resets_at_ms: Date.UTC(2026, 8, 10, 0, 0),
    },
    {
      kind: "additional",
      limit_name: "GPT-5.3-Codex-Spark",
      used_percent: 18,
      window_minutes: 300,
      resets_at_ms: Date.UTC(2026, 8, 3, 9, 33),
    },
    {
      kind: "additional",
      limit_name: "GPT-5.3-Codex-Spark",
      used_percent: 9,
      window_minutes: 10_080,
      resets_at_ms: Date.UTC(2026, 8, 10, 0, 0),
    },
  ],
};

describe("AccountsTable OpenAI accounts", () => {
  it("shows the configured name and Base URL in separate columns", () => {
    const noop = vi.fn();
    renderWithProviders(
      <AccountsTable
        accounts={[openAiAccount]}
        loading={false}
        reordering={false}
        today="2026-08-26"
        checkinsDate="2026-08-26"
        checkinDoneMap={{}}
        refreshing={{}}
        systemChecking={{}}
        pageOpening={{}}
        setAccounts={noop}
        persistOrder={async () => undefined}
        onRefreshAccount={noop}
        onOpenBaseUrl={noop}
        onSystemCheckin={noop}
        onOpenManualCheckinPrompt={noop}
        onOpenCreateManagedChannelDialog={noop}
        onOpenEdit={noop}
        onOpenDeleteDialog={noop}
      />,
    );

    expect(screen.getByText("Personal OpenAI")).toBeInTheDocument();
    expect(screen.queryByText("codex@example.com")).not.toBeInTheDocument();
    expect(screen.getByRole("columnheader", { name: "名称" })).toBeInTheDocument();
    expect(screen.getByRole("columnheader", { name: "Base URL" })).toBeInTheDocument();
    expect(screen.getByText("https://chatgpt.com")).toBeInTheDocument();
    expect(screen.getByText("无签到")).toBeInTheDocument();
  });

  it("shows the default OpenAI quota and labels additional limit buckets on hover", async () => {
    const user = userEvent.setup();
    const noop = vi.fn();
    renderWithProviders(
      <AccountsTable
        accounts={[openAiAccount]}
        loading={false}
        reordering={false}
        today="2026-08-26"
        checkinsDate="2026-08-26"
        checkinDoneMap={{}}
        refreshing={{}}
        systemChecking={{}}
        pageOpening={{}}
        setAccounts={noop}
        persistOrder={async () => undefined}
        onRefreshAccount={noop}
        onOpenBaseUrl={noop}
        onSystemCheckin={noop}
        onOpenManualCheckinPrompt={noop}
        onOpenCreateManagedChannelDialog={noop}
        onOpenEdit={noop}
        onOpenDeleteDialog={noop}
      />,
    );

    const defaultQuota = screen.getByText("43%");
    expect(defaultQuota).toBeInTheDocument();
    expect(screen.queryByText("18%")).not.toBeInTheDocument();
    expect(screen.queryByText(/重置时间/)).not.toBeInTheDocument();
    expect(screen.queryByText("7日限额 43%")).not.toBeInTheDocument();

    await user.hover(defaultQuota);

    expect((await screen.findAllByText("7日限额 43%")).length).toBeGreaterThan(0);
    expect(
      screen.getAllByText("GPT-5.3-Codex-Spark 5小时限额 18%").length,
    ).toBeGreaterThan(0);
    expect(
      screen.getAllByText("GPT-5.3-Codex-Spark 7日限额 9%").length,
    ).toBeGreaterThan(0);
    expect(screen.getAllByText("7日限额 43%").length).toBeGreaterThan(0);
    expect(screen.getAllByText(/重置时间/).length).toBeGreaterThanOrEqual(3);
    expect(screen.queryByText(/·/)).not.toBeInTheDocument();

    const usage = screen.getAllByText("GPT-5.3-Codex-Spark 5小时限额 18%")[0];
    const reset = screen.getAllByText(/重置时间/)[0];
    expect(usage.className).toContain("text-left");
    expect(reset.className).toContain("text-right");
  });
});
