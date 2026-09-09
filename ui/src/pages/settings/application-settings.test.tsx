import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen } from "@testing-library/react";
import { renderWithProviders } from "@/test/render";
import type { AppSettings } from "@/types/api";
import { getCodexNotifyCommand } from "@/api";
import { ApplicationSettings } from "./application-settings";
import { codexNotifyToml } from "./application-settings";

vi.mock("@/api", async () => ({
  ...(await vi.importActual<typeof import("@/api")>("@/api")),
  getCodexNotifyCommand: vi.fn(),
  updateSettings: vi.fn(),
}));

const settings = { desktop_pet_enabled: true } as AppSettings;

beforeEach(() => {
  Object.defineProperty(window, "ipc", { value: { postMessage: vi.fn() }, configurable: true });
  vi.mocked(getCodexNotifyCommand).mockReset();
});

describe("codex notify config", () => {
  it("serializes command paths as TOML basic strings without shell interpolation", () => {
    expect(codexNotifyToml(["/opt/Cli Switch/cliswitch", "--data-dir", "C:\\Users\\A", "--activity-notify", "a\"b\n"])).toBe(
      'notify = ["/opt/Cli Switch/cliswitch", "--data-dir", "C:\\\\Users\\\\A", "--activity-notify", "a\\\"b\\n"]',
    );
  });

  it("rejects malformed responses instead of producing a destructive config", () => {
    expect(() => codexNotifyToml(null)).toThrow("invalid_codex_notify_command");
    expect(() => codexNotifyToml(["--data-dir", 42, { token: "secret" }])).toThrow(
      "invalid_codex_notify_command",
    );
    expect(() => codexNotifyToml(["--data-dir", ""])).toThrow(
      "invalid_codex_notify_command",
    );
  });

  it("does not fetch the command until the user clicks copy", async () => {
    const clipboard = vi.fn(async () => undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText: clipboard }, configurable: true });
    vi.mocked(getCodexNotifyCommand).mockResolvedValue({ command: ["/app/cliswitch", "--data-dir", "/tmp/data", "activity-notify"] });
    renderWithProviders(<ApplicationSettings settings={settings} onSaved={vi.fn()} />);
    expect(screen.getByText("Codex 完成通知设置")).toBeInTheDocument();
    expect(getCodexNotifyCommand).not.toHaveBeenCalled();
    fireEvent.click(screen.getByText("Codex 完成通知设置"));
    expect(getCodexNotifyCommand).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "复制配置" }));
    await vi.waitFor(() => expect(getCodexNotifyCommand).toHaveBeenCalledTimes(1));
    await vi.waitFor(() => expect(clipboard).toHaveBeenCalledWith('notify = ["/app/cliswitch", "--data-dir", "/tmp/data", "activity-notify"]'));
  });
});
