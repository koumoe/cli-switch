import { installCliTool, type CliToolStatus, type InstallCliToolResponse } from "@/api";
import { humanizeApiError } from "@/lib/error";
import { toast } from "sonner";

type Translator = (key: string, vars?: Record<string, string | number>) => string;

export async function installCliToolWithToast(opts: {
  tool: CliToolStatus;
  t: Translator;
  onToolUpdated: (tool: CliToolStatus) => void;
}): Promise<InstallCliToolResponse | null> {
  const { tool, t, onToolUpdated } = opts;
  try {
    const res = await installCliTool(tool.id);
    onToolUpdated(res.tool);

    if (res.ok) {
      toast.success(t("settings.cliTools.installOk", { name: tool.name }), {
        description: res.terminal_shim_ok
          ? t("settings.cliTools.terminalReady")
          : res.terminal_shim_error
            ? t("settings.cliTools.terminalSetupFailed", { error: res.terminal_shim_error })
            : undefined,
      });
    } else {
      toast.error(t("settings.cliTools.installFail", { name: tool.name }), {
        description: res.stderr?.trim() || res.stdout?.trim() || "-",
      });
    }

    return res;
  } catch (e) {
    toast.error(t("settings.cliTools.installFail", { name: tool.name }), {
      description: humanizeApiError(e, t),
    });
    return null;
  }
}

