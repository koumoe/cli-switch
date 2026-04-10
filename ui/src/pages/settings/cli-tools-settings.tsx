import { RefreshCw } from "lucide-react";

import {
  Button,
  Switch,
} from "@/components/ui";
import { useI18n } from "@/hooks/use-i18n";
import type {
  AppSettings,
  CliToolId,
  CliToolProxyConfigStatus,
  CliToolsStatus,
} from "@/types/api";

type CliToolsSettingsProps = {
  cliToolsProxyConfig: CliToolProxyConfigStatus | null;
  cliToolsProxyConfigLoading: boolean;
  cliProxyConfigBusy: Record<CliToolId, boolean>;
  cliToolsStatus: CliToolsStatus | null;
  cliToolsLoading: boolean;
  cliToolBusy: Record<CliToolId, boolean>;
  appSettings: AppSettings | null;
  onRefreshCliToolsProxyConfigStatus: () => void | Promise<void>;
  onApplyCliProxyConfig: (toolId: CliToolId) => void | Promise<void>;
  onRefreshCliToolsStatus: () => void | Promise<void>;
  onInstallCliTool: (toolId: CliToolId) => void | Promise<void>;
  onCliToolAutoUpdateChange: (toolId: CliToolId, enabled: boolean) => void | Promise<void>;
};

export function CliToolsSettings({
  cliToolsProxyConfig,
  cliToolsProxyConfigLoading,
  cliProxyConfigBusy,
  cliToolsStatus,
  cliToolsLoading,
  cliToolBusy,
  appSettings,
  onRefreshCliToolsProxyConfigStatus,
  onApplyCliProxyConfig,
  onRefreshCliToolsStatus,
  onInstallCliTool,
  onCliToolAutoUpdateChange,
}: CliToolsSettingsProps) {
  const { t } = useI18n();

  return (
    <div className="pb-4">
      <div className="flex items-center justify-between gap-3 border-t border-slate-100 px-5 pb-1 pt-2.5 dark:border-slate-800/40">
        <div className="text-[10px] font-bold uppercase tracking-[0.06em] text-slate-400 dark:text-slate-500">
          {t("settings.cliProxyConfig.title")}
        </div>
        <Button
          size="sm"
          variant="outline"
          onClick={() => void onRefreshCliToolsProxyConfigStatus()}
          disabled={cliToolsProxyConfigLoading}
          className="h-7 gap-1.5 rounded-md px-2 text-[11px]"
        >
          <RefreshCw className={`h-3.5 w-3.5 ${cliToolsProxyConfigLoading ? "animate-spin" : ""}`} />
          {t("settings.cliProxyConfig.refresh")}
        </Button>
      </div>
      {!cliToolsProxyConfig ? (
        <div className="border-t border-slate-100 px-5 py-3 text-[11px] text-slate-500 dark:border-slate-800/40 dark:text-slate-400">
          {cliToolsProxyConfigLoading ? t("common.loading") : "-"}
        </div>
      ) : null}
      {(cliToolsProxyConfig?.tools ?? []).map((tool) => {
        const busy = cliProxyConfigBusy[tool.id];
        return (
          <div
            key={tool.id}
            className="flex min-h-[50px] items-center justify-between gap-4 border-t border-slate-100 px-5 py-3 transition-colors hover:bg-blue-50/25 dark:border-slate-800/40 dark:hover:bg-slate-800/25"
          >
            <div className="min-w-0 flex-1">
              <div className="text-[12.5px] font-semibold">{tool.name}</div>
              <div className="mt-0.5 text-[10.5px] text-slate-500 dark:text-slate-400">
                {tool.ok ? t("settings.cliProxyConfig.ok") : t("settings.cliProxyConfig.needsFix")}
              </div>
            </div>
            <Button
              size="sm"
              disabled={tool.ok || busy}
              onClick={() => void onApplyCliProxyConfig(tool.id)}
              className="h-7 rounded-md px-2 text-[11px]"
            >
              {t("settings.cliProxyConfig.apply")}
            </Button>
          </div>
        );
      })}

      <div className="flex items-center justify-between gap-3 border-t border-slate-100 px-5 pb-1 pt-2.5 dark:border-slate-800/40">
        <div className="text-[10px] font-bold uppercase tracking-[0.06em] text-slate-400 dark:text-slate-500">
          {t("settings.cliTools.title")}
        </div>
        <Button
          size="sm"
          variant="outline"
          onClick={() => void onRefreshCliToolsStatus()}
          disabled={cliToolsLoading}
          className="h-7 gap-1.5 rounded-md px-2 text-[11px]"
        >
          <RefreshCw className={`h-3.5 w-3.5 ${cliToolsLoading ? "animate-spin" : ""}`} />
          {t("settings.cliTools.refresh")}
        </Button>
      </div>
      {!cliToolsStatus ? (
        <div className="border-t border-slate-100 px-5 py-3 text-[11px] text-slate-500 dark:border-slate-800/40 dark:text-slate-400">
          {cliToolsLoading ? t("common.loading") : "-"}
        </div>
      ) : null}
      {(cliToolsStatus?.tools ?? []).map((tool) => {
        const installed = tool.installed;
        const version = tool.version ?? "-";
        const busy = cliToolBusy[tool.id];
        const autoEnabled =
          tool.id === "gemini"
            ? (appSettings?.gemini_cli_auto_update_enabled ?? false)
            : tool.id === "claude"
              ? (appSettings?.claude_code_auto_update_enabled ?? false)
              : (appSettings?.codex_auto_update_enabled ?? false);

        return (
          <div
            key={tool.id}
            className="flex min-h-[50px] items-center justify-between gap-4 border-t border-slate-100 px-5 py-3 transition-colors hover:bg-blue-50/25 dark:border-slate-800/40 dark:hover:bg-slate-800/25"
          >
            <div className="min-w-0 flex-1">
              <div className="text-[12.5px] font-semibold">{tool.name}</div>
              <div className="mt-0.5 text-[10.5px] text-slate-500 dark:text-slate-400">
                {installed
                  ? t("settings.cliTools.installedWithVersion", { version })
                  : t("settings.cliTools.notInstalled")}
              </div>
            </div>
            <div className="flex shrink-0 items-center gap-3">
              <Button
                size="sm"
                variant="outline"
                disabled={busy}
                onClick={() => void onInstallCliTool(tool.id)}
                className="h-7 rounded-md px-2 text-[11px]"
              >
                {installed ? t("settings.cliTools.update") : t("settings.cliTools.install")}
              </Button>
              <div className="flex items-center gap-2">
                <div className="text-[10.5px] text-slate-500 dark:text-slate-400">
                  {t("settings.cliTools.autoEnable")}
                </div>
                <Switch
                  checked={autoEnabled}
                  onCheckedChange={(value) => {
                    void onCliToolAutoUpdateChange(tool.id, value);
                  }}
                  disabled={!appSettings}
                />
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
}
