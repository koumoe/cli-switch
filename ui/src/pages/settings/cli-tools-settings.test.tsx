import type { ComponentProps } from "react";
import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { formatDateTime } from "@/lib/format";
import { cliStatus, cliTool } from "@/test/fixtures/cli-tools";
import { renderWithProviders } from "@/test/render";
import { CliToolsSettings } from "./cli-tools-settings";

function props(
  overrides: Partial<ComponentProps<typeof CliToolsSettings>> = {},
): ComponentProps<typeof CliToolsSettings> {
  return {
    cliToolsProxyConfig: null,
    cliToolsProxyConfigLoading: false,
    cliProxyConfigBusy: { codex: false, claude: false, gemini: false },
    cliToolsStatus: cliStatus(),
    cliToolsLoading: false,
    cliToolsError: null,
    cliToolBusy: { codex: false, claude: false, gemini: false },
    appSettings: null,
    onRefreshCliToolsProxyConfigStatus: vi.fn(),
    onApplyCliProxyConfig: vi.fn(),
    onRefreshCliToolsStatus: vi.fn(),
    onInstallCliTool: vi.fn(),
    onCliToolAutoUpdateChange: vi.fn(),
    ...overrides,
  };
}

describe("CliToolsSettings", () => {
  it("disables updates when no newer version is available", () => {
    const options = props({
      cliToolsStatus: cliStatus(cliTool({ latest_version: "1.0.0", update_available: false })),
    });
    renderWithProviders(<CliToolsSettings {...options} />);

    const update = screen.getByRole("button", { name: "更新" });
    expect(update).toBeDisabled();
    fireEvent.click(update);
    expect(options.onInstallCliTool).not.toHaveBeenCalled();
    expect(screen.getByText("暂无可用更新")).toBeInTheDocument();
  });

  it("shows the target version and updates the selected tool", () => {
    const options = props();
    renderWithProviders(<CliToolsSettings {...options} />);

    expect(screen.getByText("有可用更新：1.1.0")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "更新" }));
    expect(options.onInstallCliTool).toHaveBeenCalledWith("codex");
  });

  it("keeps installation available for a missing tool without an update", () => {
    const options = props({
      cliToolsStatus: cliStatus(cliTool({ installed: false, version: null, update_available: false })),
    });
    renderWithProviders(<CliToolsSettings {...options} />);

    expect(screen.getByText("未安装")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "安装" }));
    expect(options.onInstallCliTool).toHaveBeenCalledWith("codex");
  });

  it.each(["local", "backend"])("shows and disables an update started by %s", (source) => {
    renderWithProviders(<CliToolsSettings {...props({
      cliToolsStatus: cliStatus(cliTool({ updating: source === "backend" })),
      cliToolBusy: { codex: source === "local", gemini: false, claude: false },
    })} />);

    expect(screen.getByRole("button", { name: "更新中..." })).toBeDisabled();
    expect(screen.getByText("正在更新至 1.1.0...")).toBeInTheDocument();
  });

  it("shows installation progress for a missing tool", () => {
    renderWithProviders(<CliToolsSettings {...props({
      cliToolsStatus: cliStatus(cliTool({ installed: false, version: null })),
      cliToolBusy: { codex: true, gemini: false, claude: false },
    })} />);

    expect(screen.getByRole("button", { name: "安装中..." })).toBeDisabled();
  });

  it("disables actions and replaces the previous result while checking", () => {
    const options = props({ cliToolsLoading: true });
    renderWithProviders(<CliToolsSettings {...options} />);

    expect(screen.getByRole("button", { name: "检测中..." })).toBeDisabled();
    const update = screen.getByRole("button", { name: "更新" });
    expect(update).toBeDisabled();
    fireEvent.click(update);
    expect(options.onInstallCliTool).not.toHaveBeenCalled();
    expect(screen.queryByText("有可用更新：1.1.0")).not.toBeInTheDocument();
  });

  it("shows a retry message instead of claiming a failed check is current", () => {
    const options = props({
      cliToolsStatus: cliStatus(cliTool({
        latest_version: "1.0.0",
        update_available: false,
        update_check_error: "Registry unavailable",
      })),
    });
    renderWithProviders(<CliToolsSettings {...options} />);

    expect(screen.getByText("检测更新失败，请刷新重试。")).toBeInTheDocument();
    expect(screen.queryByText("Registry unavailable")).not.toBeInTheDocument();
    expect(screen.queryByText("暂无可用更新")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "更新" })).toBeDisabled();
    fireEvent.click(screen.getAllByRole("button", { name: "刷新" })[1]);
    expect(options.onRefreshCliToolsStatus).toHaveBeenCalledOnce();
  });

  it.each([
    { latest_version: null },
    { version: null },
  ])("does not claim unknown versions are current: %o", (unknownVersion) => {
    renderWithProviders(<CliToolsSettings {...props({
      cliToolsStatus: cliStatus(cliTool({ update_available: false, ...unknownVersion })),
    })} />);

    expect(screen.getByText("更新状态未知，请刷新检测。")).toBeInTheDocument();
    expect(screen.queryByText("暂无可用更新")).not.toBeInTheDocument();
  });

  it("marks a failed cache refresh while preserving a previously available update", () => {
    const options = props({ cliToolsError: "Network unavailable" });
    renderWithProviders(<CliToolsSettings {...options} />);

    expect(screen.getByText("检测更新失败，请刷新重试。")).toBeInTheDocument();
    expect(screen.queryByText("Network unavailable")).not.toBeInTheDocument();
    expect(screen.queryByText("暂无可用更新")).not.toBeInTheDocument();
    const update = screen.getByRole("button", { name: "更新" });
    expect(update).toBeEnabled();
    fireEvent.click(update);
    expect(options.onInstallCliTool).toHaveBeenCalledWith("codex");
  });

  it("keeps request failures visible with a retry action when no status was loaded", () => {
    renderWithProviders(<CliToolsSettings {...props({
      cliToolsStatus: null,
      cliToolsError: "Network unavailable",
    })} />);

    expect(screen.getByText(/无法获取 CLI 状态，请刷新重试。/)).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "刷新" })[1]).toBeEnabled();
  });

  it.each(["zh-CN", "en-US"] as const)("localizes the schedule and Unix check time in %s", (locale) => {
    renderWithProviders(<CliToolsSettings {...props()} />, { locale });
    const time = formatDateTime(1_700_000_000 * 1000, { locale });

    expect(screen.getByText(locale === "zh-CN"
      ? "启动时及每 6 小时在后台检测更新；开启自动更新后，发现新版本会自动更新。"
      : "Checks for updates at startup and every 6 hours in the background. With auto update enabled, new versions are installed when found."
    )).toBeInTheDocument();
    expect(screen.getByText(locale === "zh-CN" ? `上次检测：${time}` : `Last checked: ${time}`)).toBeInTheDocument();
  });
});
