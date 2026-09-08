import { act } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { toast } from "sonner";

import { resetOpenAiAccountQuota } from "@/api";
import { renderWithProviders } from "@/test/render";
import type { OpenAiQuotaResetResult, RemoteAccount } from "@/types/api";

import { openAiAccount } from "./test-fixtures";
import { useQuotaReset } from "./useQuotaReset";

vi.mock("@/api", () => ({ resetOpenAiAccountQuota: vi.fn() }));
vi.mock("sonner", () => ({ toast: { success: vi.fn(), error: vi.fn(), info: vi.fn(), warning: vi.fn() } }));

const consume = vi.mocked(resetOpenAiAccountQuota);
function response(outcome: OpenAiQuotaResetResult["outcome"] = "reset", error: string | null = null): OpenAiQuotaResetResult {
  return {
    outcome,
    account: { ...openAiAccount, quota_reset_available_count: 1 },
    quota_refresh_error: error,
  };
}

function mountReset() {
  let current: ReturnType<typeof useQuotaReset>;
  const setAccounts = vi.fn();
  function Harness() {
    current = useQuotaReset(setAccounts);
    return null;
  }
  const view = renderWithProviders(<Harness />);
  return { ...view, setAccounts, get current() { return current; } };
}

beforeEach(() => { localStorage.clear(); vi.clearAllMocks(); });

describe("quota reset transaction", () => {
  it("reuses the key after an uncertain request across unmounts, even when the count becomes zero", async () => {
    consume.mockRejectedValueOnce(new TypeError("Failed to fetch"));
    const first = mountReset();
    await act(async () => { expect(await first.current.resetQuota(openAiAccount)).toBe(false); });
    const key = consume.mock.calls[0][1];
    expect(first.current.pending[openAiAccount.id]).toBe(true);
    first.unmount();

    consume.mockResolvedValueOnce(response("already_redeemed"));
    const second = mountReset();
    await act(async () => {
      expect(await second.current.resetQuota({ ...openAiAccount, quota_reset_available_count: 0 })).toBe(true);
    });
    expect(consume.mock.calls[1][1]).toBe(key);
    expect(second.current.pending[openAiAccount.id]).toBeUndefined();
    expect(toast.success).toHaveBeenCalledWith("用量已重置");

    consume.mockResolvedValueOnce(response());
    await act(async () => { await second.current.resetQuota(openAiAccount); });
    expect(consume.mock.calls[2][1]).not.toBe(key);
  });

  it("rejects duplicate clicks before React has applied the busy state", async () => {
    let resolve!: (value: OpenAiQuotaResetResult) => void;
    consume.mockImplementationOnce(() => new Promise((done) => { resolve = done; }));
    const view = mountReset();
    let pending!: Promise<boolean>;
    act(() => { pending = view.current.resetQuota(openAiAccount); });
    await act(async () => { expect(await view.current.resetQuota(openAiAccount)).toBe(false); });
    expect(consume).toHaveBeenCalledOnce();
    await act(async () => { resolve(response()); await pending; });
  });

  it("blocks another consumption after success with failed synchronization until a successful refresh", async () => {
    consume.mockResolvedValueOnce(response("reset", "usage endpoint unavailable"));
    const view = mountReset();
    await act(async () => { expect(await view.current.resetQuota(openAiAccount)).toBe(true); });
    expect(view.current.refreshRequired[openAiAccount.id]).toBe(true);
    expect(toast.warning).toHaveBeenCalledWith("重置成功，用量同步失败，请刷新账号");
    await act(async () => { expect(await view.current.resetQuota(openAiAccount)).toBe(false); });
    expect(consume).toHaveBeenCalledOnce();
    act(() => { view.current.markQuotaRefreshed({ ...openAiAccount, last_sync_error: "still unavailable" }); });
    expect(view.current.refreshRequired[openAiAccount.id]).toBe(true);
    act(() => { view.current.markQuotaRefreshed({ ...openAiAccount, last_sync_error: null }); });
    expect(view.current.refreshRequired[openAiAccount.id]).toBeUndefined();
    expect(view.current.getRefreshRequiredIds()).toEqual([]);
  });

  it.each(["nothing_to_reset", "no_credit"] as const)("uses the returned snapshot and does not report reset success for %s", async (outcome) => {
    const result = response(outcome);
    consume.mockResolvedValueOnce(result);
    const view = mountReset();
    await act(async () => { expect(await view.current.resetQuota(openAiAccount)).toBe(true); });
    expect(toast.success).not.toHaveBeenCalled();
    expect(toast.info).toHaveBeenCalledOnce();
    const updater = view.setAccounts.mock.calls[0][0] as (accounts: RemoteAccount[]) => RemoteAccount[];
    expect(updater([openAiAccount])[0]).toBe(result.account);
    expect(view.current.pending[openAiAccount.id]).toBeUndefined();
  });

  it.each([0, null, undefined])("does not consume with %s cards when no previous attempt exists", async (count) => {
    const view = mountReset();
    await act(async () => { expect(await view.current.resetQuota({ ...openAiAccount, quota_reset_available_count: count })).toBe(false); });
    expect(consume).not.toHaveBeenCalled();
  });
});
