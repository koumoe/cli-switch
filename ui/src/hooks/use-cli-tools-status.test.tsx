import type { ReactNode } from "react";
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { toast } from "sonner";

import { getCliToolsStatus, installCliTool } from "@/api";
import { I18nContext, translateForLocale } from "@/providers/i18n-provider";
import { cliInstallResult, cliStatus, cliTool } from "@/test/fixtures/cli-tools";
import type { CliToolsStatus, InstallCliToolResponse } from "@/types/api";
import { useCliToolsStatus } from "./use-cli-tools-status";

vi.mock("@/api", () => ({
  getCliToolsStatus: vi.fn(),
  installCliTool: vi.fn(),
}));
vi.mock("sonner", () => ({ toast: { error: vi.fn(), success: vi.fn() } }));

const i18n = {
  locale: "en-US" as const,
  setLocale: vi.fn(),
  t: (key: string, vars?: Record<string, string | number>) => translateForLocale("en-US", key, vars),
  locales: [],
};

function Wrapper({ children }: { children: ReactNode }) {
  return <I18nContext.Provider value={i18n}>{children}</I18nContext.Provider>;
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

async function mount() {
  const hook = renderHook(() => useCliToolsStatus(), { wrapper: Wrapper });
  await act(async () => {});
  return hook;
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.mocked(getCliToolsStatus).mockReset().mockResolvedValue(cliStatus());
  vi.mocked(installCliTool).mockReset().mockResolvedValue(cliInstallResult());
});

afterEach(() => {
  vi.useRealTimers();
});

describe("useCliToolsStatus", () => {
  it("reads cached status on entry and every 30 seconds without foreground loading", async () => {
    const { result, unmount } = await mount();
    expect(getCliToolsStatus).toHaveBeenNthCalledWith(1);
    expect(result.current.status).toEqual(cliStatus());

    const poll = deferred<CliToolsStatus>();
    vi.mocked(getCliToolsStatus).mockReturnValueOnce(poll.promise);
    await act(async () => vi.advanceTimersByTimeAsync(29_999));
    expect(getCliToolsStatus).toHaveBeenCalledTimes(1);
    await act(async () => vi.advanceTimersByTimeAsync(1));
    expect(getCliToolsStatus).toHaveBeenNthCalledWith(2);
    expect(result.current.loading).toBe(false);
    await act(async () => poll.resolve(cliStatus(cliTool({ updating: true }))));
    expect(result.current.status?.tools[0].updating).toBe(true);

    unmount();
    expect(vi.getTimerCount()).toBe(0);
    await act(async () => vi.advanceTimersByTimeAsync(60_000));
    expect(getCliToolsStatus).toHaveBeenCalledTimes(2);
  });

  it.each(["success", "error"])("manual refresh supersedes an older poll's %s", async (outcome) => {
    const { result } = await mount();
    const poll = deferred<CliToolsStatus>();
    const manual = deferred<CliToolsStatus>();
    vi.mocked(getCliToolsStatus).mockReturnValueOnce(poll.promise).mockReturnValueOnce(manual.promise);
    await act(async () => vi.advanceTimersByTimeAsync(30_000));
    let refresh!: Promise<void>;
    act(() => { refresh = result.current.refresh(); });
    expect(getCliToolsStatus).toHaveBeenLastCalledWith({ refresh: true });
    expect(result.current.loading).toBe(true);
    await act(async () => vi.advanceTimersByTimeAsync(60_000));
    expect(getCliToolsStatus).toHaveBeenCalledTimes(3);

    const latest = cliStatus(cliTool({ version: "1.1.0", update_available: false }));
    await act(async () => { manual.resolve(latest); await refresh; });
    await act(async () => {
      if (outcome === "success") poll.resolve(cliStatus());
      else poll.reject(new Error("Stale failure"));
    });
    expect(result.current.status).toEqual(latest);
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
    expect(toast.error).not.toHaveBeenCalled();
  });

  it("does not finish checking when a superseded cache response arrives first", async () => {
    const initial = deferred<CliToolsStatus>();
    const manual = deferred<CliToolsStatus>();
    vi.mocked(getCliToolsStatus).mockReturnValueOnce(initial.promise).mockReturnValueOnce(manual.promise);
    const { result } = await mount();
    act(() => { void result.current.refresh(); });
    await act(async () => initial.resolve(cliStatus()));
    expect(result.current.loading).toBe(true);
    expect(result.current.status).toBeNull();
    await act(async () => manual.resolve(cliStatus()));
    expect(result.current.loading).toBe(false);
  });

  it.each(["during", "after"])("ignores an old poll resolved %s installation", async (timing) => {
    const { result } = await mount();
    const poll = deferred<CliToolsStatus>();
    const installation = deferred<InstallCliToolResponse>();
    vi.mocked(getCliToolsStatus).mockReturnValueOnce(poll.promise);
    vi.mocked(installCliTool).mockReturnValueOnce(installation.promise);
    await act(async () => vi.advanceTimersByTimeAsync(30_000));
    let install!: Promise<void>;
    act(() => { install = result.current.install("codex"); });
    expect(result.current.busy.codex).toBe(true);
    await act(async () => result.current.install("codex"));
    expect(installCliTool).toHaveBeenCalledTimes(1);
    expect(getCliToolsStatus).toHaveBeenCalledTimes(2);
    if (timing === "during") await act(async () => poll.resolve(cliStatus()));
    await act(async () => { installation.resolve(cliInstallResult()); await install; });
    if (timing === "after") await act(async () => poll.resolve(cliStatus()));

    expect(result.current.status?.tools[0]).toEqual(cliInstallResult().tool);
    expect(result.current.busy.codex).toBe(false);
    vi.mocked(getCliToolsStatus).mockResolvedValue(cliStatus(cliTool({ latest_version: "1.2.0" })));
    await act(async () => vi.advanceTimersByTimeAsync(30_000));
    expect(result.current.status?.tools[0].latest_version).toBe("1.2.0");
  });

  it("polls during installation and prevents a pending refresh from overwriting its result", async () => {
    const { result } = await mount();
    const installation = deferred<InstallCliToolResponse>();
    vi.mocked(installCliTool).mockReturnValueOnce(installation.promise);
    act(() => { void result.current.install("codex"); });
    vi.mocked(getCliToolsStatus).mockResolvedValue(cliStatus(cliTool({ updating: true })));
    await act(async () => vi.advanceTimersByTimeAsync(30_000));
    expect(getCliToolsStatus).toHaveBeenCalledTimes(2);
    expect(result.current.status?.tools[0].updating).toBe(true);
    expect(result.current.busy.codex).toBe(true);

    const refresh = deferred<CliToolsStatus>();
    vi.mocked(getCliToolsStatus).mockReturnValueOnce(refresh.promise);
    act(() => { void result.current.refresh(); });
    await act(async () => installation.resolve(cliInstallResult()));
    await act(async () => refresh.resolve(cliStatus(cliTool({ updating: true }))));
    expect(result.current.status?.tools[0]).toEqual(cliInstallResult().tool);
    expect(result.current.busy.codex).toBe(false);
    expect(result.current.loading).toBe(false);
  });

  it("keeps polling failures quiet and recovers on the next cache read", async () => {
    const { result } = await mount();
    vi.mocked(getCliToolsStatus).mockRejectedValueOnce(new Error("Offline"));
    await act(async () => vi.advanceTimersByTimeAsync(30_000));
    expect(result.current.status).toEqual(cliStatus());
    expect(result.current.error).toBe("Offline");
    expect(toast.error).not.toHaveBeenCalled();
    await act(async () => vi.advanceTimersByTimeAsync(30_000));
    expect(result.current.error).toBeNull();
  });

  it("reports a manual request failure and allows retry", async () => {
    const { result } = await mount();
    vi.mocked(getCliToolsStatus).mockRejectedValueOnce(new Error("Offline"));
    await act(async () => result.current.refresh());
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBe("Offline");
    expect(toast.error).toHaveBeenCalledWith("Failed to load CLI status", { description: "Offline" });
    await act(async () => result.current.refresh());
    expect(result.current.error).toBeNull();
  });

  it.each([
    { update_available: false },
    { updating: true },
  ])("rejects ineligible update actions: %o", async (toolState) => {
    vi.mocked(getCliToolsStatus).mockResolvedValue(cliStatus(cliTool(toolState)));
    const { result } = await mount();
    await act(async () => result.current.install("codex"));
    expect(installCliTool).not.toHaveBeenCalled();
  });

  it("blocks installation while a manual check is pending", async () => {
    const { result } = await mount();
    const manual = deferred<CliToolsStatus>();
    vi.mocked(getCliToolsStatus).mockReturnValueOnce(manual.promise);
    act(() => { void result.current.refresh(); });
    await act(async () => result.current.install("codex"));
    expect(installCliTool).not.toHaveBeenCalled();
    await act(async () => manual.resolve(cliStatus()));
  });

  it("installs missing tools even when update_available is false", async () => {
    vi.mocked(getCliToolsStatus).mockResolvedValue(cliStatus(cliTool({
      installed: false, version: null, update_available: false,
    })));
    const { result } = await mount();
    await act(async () => result.current.install("codex"));
    expect(installCliTool).toHaveBeenCalledWith("codex");
    expect(result.current.status?.tools[0].installed).toBe(true);
  });

  it("releases local busy state after a failed installation", async () => {
    const { result } = await mount();
    vi.mocked(installCliTool).mockRejectedValueOnce(new Error("Install failed"));
    await act(async () => result.current.install("codex"));
    expect(result.current.busy.codex).toBe(false);
    expect(toast.error).toHaveBeenCalledOnce();
    await act(async () => result.current.install("codex"));
    expect(result.current.status?.tools[0].version).toBe("1.1.0");
  });

  it("ignores obsolete startup requests after StrictMode effect cleanup", async () => {
    const obsolete = deferred<CliToolsStatus>();
    vi.mocked(getCliToolsStatus).mockReturnValueOnce(obsolete.promise);
    const { result, unmount } = renderHook(() => useCliToolsStatus(), {
      wrapper: Wrapper,
      reactStrictMode: true,
    });
    await act(async () => {});
    expect(getCliToolsStatus).toHaveBeenCalledTimes(2);
    await act(async () => obsolete.reject(new Error("Obsolete startup failure")));
    expect(result.current.status).toEqual(cliStatus());
    expect(result.current.error).toBeNull();
    expect(toast.error).not.toHaveBeenCalled();
    unmount();
    expect(vi.getTimerCount()).toBe(0);
  });
});
