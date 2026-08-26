import { screen } from "@testing-library/react";
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
      kind: "weekly",
      used_percent: 43,
      window_minutes: 10_080,
      resets_at_ms: null,
    },
    {
      kind: "primary",
      used_percent: 18,
      window_minutes: 300,
      resets_at_ms: null,
    },
  ],
};

describe("AccountsTable OpenAI accounts", () => {
  it("shows native identity, quota windows, and unsupported check-in", () => {
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

    expect(screen.getAllByText("codex@example.com").length).toBeGreaterThan(0);
    expect(screen.getByText("周限额 43%")).toBeInTheDocument();
    expect(screen.getByText("5 小时限额 18%")).toBeInTheDocument();
    expect(screen.getByText("不支持签到")).toBeInTheDocument();
  });
});
