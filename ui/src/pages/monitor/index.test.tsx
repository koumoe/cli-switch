import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { listRemoteAccounts } from "@/api";
import { renderWithProviders } from "@/test/render";
import type { RemoteAccount } from "@/types/api";
import { MonitorPage } from "./index";

vi.mock("@/api", async () => ({
  ...await vi.importActual<typeof import("@/api")>("@/api"),
  getUsdCnyExchangeRate: vi.fn(async () => ({ rate: 6.72 })),
  listChannels: vi.fn(async () => [
    { id: "a", name: "OpenAI 20X", managed_remote_account_id: "account-a" },
    { id: "b", name: "OpenAI 20X", managed_remote_account_id: "account-b" },
    { id: "c", name: "Unbound", managed_remote_account_id: null },
  ]),
  listRemoteAccounts: vi.fn(),
  statsSummary: vi.fn(async () => ({ requests: 31, success: 30, failed: 1, estimated_cost_usd: null })),
  statsChannels: vi.fn(async () => ({ items: [
    { channel_id: "a", name: "OpenAI 20X", protocol: "openai", requests: 21, success: 20, failed: 1, avg_latency_ms: 150, estimated_cost_usd: null },
    { channel_id: "b", name: "OpenAI 20X", protocol: "openai", requests: 9, success: 9, failed: 0, avg_latency_ms: 180, estimated_cost_usd: null },
    { channel_id: "c", name: "Unbound", protocol: "openai", requests: 1, success: 1, failed: 0, avg_latency_ms: 100, estimated_cost_usd: null },
  ] })),
}));

const accounts = [
  { id: "account-a", name: "Zeta", remote_username: "do-not-use-username" },
  { id: "account-b", name: "Alpha" },
] as RemoteAccount[];

beforeEach(() => {
  vi.mocked(listRemoteAccounts).mockResolvedValue(accounts);
});

describe("channel statistics account identity", () => {
  it("keeps equal channel names as separate rows and sorts the account column independently", async () => {
    const user = userEvent.setup();
    renderWithProviders(<MonitorPage />);
    await screen.findByText("Zeta");
    expect(screen.getAllByText("OpenAI 20X")).toHaveLength(2);
    expect(screen.queryByText("do-not-use-username")).not.toBeInTheDocument();
    const firstRow = screen.getAllByRole("row")[1];
    expect(within(firstRow).getByText("Zeta")).toBeInTheDocument();
    const accountHeader = screen.getAllByRole("columnheader")[0];
    await user.click(within(accountHeader).getByRole("button"));
    const names = screen.getAllByRole("row").slice(1).map((row) => within(row).getAllByRole("cell")[0].textContent);
    expect(names.indexOf("Alpha")).toBeLessThan(names.indexOf("Zeta"));
    expect(names).toContain("—");
  });

  it("refreshes renamed accounts and clears an unresolved mapping when metadata loading fails", async () => {
    renderWithProviders(<MonitorPage />);
    await screen.findByText("Zeta");
    vi.mocked(listRemoteAccounts).mockResolvedValue([{ ...accounts[0], name: "Renamed" }, accounts[1]]);
    fireEvent(window, new Event("cliswitch-accounts-changed"));
    await screen.findByText("Renamed");
    expect(screen.queryByText("Zeta")).not.toBeInTheDocument();

    vi.mocked(listRemoteAccounts).mockRejectedValue(new Error("Metadata unavailable"));
    await userEvent.setup().click(screen.getByRole("button", { name: "刷新" }));
    await screen.findByText("Unbound");
    expect(screen.queryByText("Renamed")).not.toBeInTheDocument();
    const names = screen.getAllByRole("row").slice(1).map((row) => within(row).getAllByRole("cell")[0].textContent);
    expect(names).toEqual(["—", "—", "—"]);
    expect(screen.getAllByText("OpenAI 20X")).toHaveLength(2);
  });

  it("does not let an older metadata response overwrite a refreshed account name", async () => {
    let resolveOld: (value: RemoteAccount[]) => void = () => undefined;
    vi.mocked(listRemoteAccounts).mockImplementationOnce(() => new Promise((resolve) => {
      resolveOld = resolve;
    }));
    renderWithProviders(<MonitorPage />);
    await waitFor(() => expect(listRemoteAccounts).toHaveBeenCalledTimes(1));
    vi.mocked(listRemoteAccounts).mockResolvedValue([{ ...accounts[0], name: "Latest name" }, accounts[1]]);
    fireEvent(window, new Event("cliswitch-accounts-changed"));
    await screen.findByText("Latest name");
    await act(async () => { resolveOld(accounts); });
    expect(screen.getByText("Latest name")).toBeInTheDocument();
    expect(screen.queryByText("Zeta")).not.toBeInTheDocument();
  });
});
