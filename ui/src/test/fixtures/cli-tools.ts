import type { CliToolStatus, CliToolsStatus, InstallCliToolResponse } from "@/types/api";

export function cliTool(overrides: Partial<CliToolStatus> = {}): CliToolStatus {
  return {
    id: "codex",
    name: "Codex",
    bin: "codex",
    npm_package: "@openai/codex",
    installed: true,
    version: "1.0.0",
    latest_version: "1.1.0",
    update_available: true,
    update_check_error: null,
    updating: false,
    install_method: "npm",
    install_path: "/usr/local/bin/codex",
    installer_path: "/usr/local/bin/npm",
    ...overrides,
  };
}

export function cliStatus(tool = cliTool()): CliToolsStatus {
  return { os: "macos", checked_at: 1_700_000_000, tools: [tool] };
}

export function cliInstallResult(
  tool = cliTool({ version: "1.1.0", update_available: false }),
): InstallCliToolResponse {
  return {
    ok: true,
    exit_code: 0,
    stdout: "",
    stderr: "",
    tool,
    terminal_shim_ok: true,
    terminal_shim_dir: "/usr/local/bin",
  };
}
