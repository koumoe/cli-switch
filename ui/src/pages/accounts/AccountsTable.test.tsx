import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { renderWithProviders } from "@/test/render";

import { AccountsTable } from "./AccountsTable";
import { openAiAccount } from "./test-fixtures";

function renderTable() {
  const noop = vi.fn();
  return renderWithProviders(
    <AccountsTable
      accounts={[openAiAccount]}
      loading={false}
      reordering={false}
      today="2026-09-08"
      checkinsDate="2026-09-08"
      checkinDoneMap={{}}
      refreshing={{}}
      systemChecking={{}}
      pageOpening={{}}
      resetting={{}}
      resetPending={{}}
      resetRefreshRequired={{}}
      onResetQuota={async () => true}
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
}

afterEach(() => vi.useRealTimers());

describe("AccountsTable OpenAI accounts", () => {
  it("shows the configured name, Base URL and reset action without a card count", () => {
    renderTable();
    expect(screen.getByText("Personal OpenAI")).toBeInTheDocument();
    expect(screen.queryByText("codex@example.com")).not.toBeInTheDocument();
    expect(screen.getByRole("columnheader", { name: "名称" })).toBeInTheDocument();
    expect(screen.getByRole("columnheader", { name: "Base URL" })).toBeInTheDocument();
    expect(screen.getByRole("columnheader", { name: "签到 / 重置" })).toBeInTheDocument();
    expect(screen.getByText("https://chatgpt.com")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "重置" })).toHaveTextContent(/^重置$/);
    expect(screen.queryByText("无签到")).not.toBeInTheDocument();
  });

  it("keeps only the main percentage in the row and groups the hover details without a duplicate name", async () => {
    vi.setSystemTime(new Date("2026-09-08T00:00:00Z"));
    const user = userEvent.setup();
    renderTable();
    const defaultQuota = screen.getByRole("button", { name: "已用 43%，查看额度详情" });
    expect(defaultQuota).toHaveTextContent(/^43%$/);
    expect(screen.queryByText("18%")).not.toBeInTheDocument();
    expect(screen.queryByText("重置时间")).not.toBeInTheDocument();

    await user.hover(defaultQuota);
    const pools = await screen.findAllByText("GPT-5.3-Codex-Spark");
    expect(pools[0].closest("td")).toHaveAttribute("rowspan", "2");
    expect(screen.getAllByText("额度池")[0]).toBeInTheDocument();
    expect(screen.getAllByText("主额度")[0]).toBeInTheDocument();
    expect(screen.getAllByText("5小时")[0]).toBeInTheDocument();
    expect(screen.getAllByText("7天")[0]).toBeInTheDocument();
    expect(screen.getAllByText("待同步")[0]).toBeInTheDocument();
    expect(screen.getAllByText("Personal OpenAI")).toHaveLength(1);
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();

    await user.unhover(defaultQuota);
    await user.keyboard("{Escape}");
    expect(screen.queryByText("额度池")).not.toBeInTheDocument();
  });
});
