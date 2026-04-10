import { Power, RefreshCw } from "lucide-react";

import {
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
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
    <div className="space-y-4">
      <Card>
        <CardHeader>
          <div className="flex items-start justify-between gap-3">
            <div className="space-y-1">
              <CardTitle className="flex items-center gap-2">
                <Power className="h-4 w-4" />
                {t("settings.cliProxyConfig.title")}
              </CardTitle>
              <CardDescription>{t("settings.cliProxyConfig.subtitle")}</CardDescription>
            </div>
            <Button
              size="sm"
              variant="outline"
              onClick={() => void onRefreshCliToolsProxyConfigStatus()}
              disabled={cliToolsProxyConfigLoading}
              className="gap-2"
            >
              <RefreshCw className={`h-4 w-4 ${cliToolsProxyConfigLoading ? "animate-spin" : ""}`} />
              {t("settings.cliProxyConfig.refresh")}
            </Button>
          </div>
        </CardHeader>
        <CardContent className="space-y-3">
          {!cliToolsProxyConfig ? (
            <div className="text-sm text-muted-foreground">
              {cliToolsProxyConfigLoading ? t("common.loading") : "-"}
            </div>
          ) : null}

          {(cliToolsProxyConfig?.tools ?? []).map((tool) => {
            const busy = cliProxyConfigBusy[tool.id];

            return (
              <div
                key={tool.id}
                className="flex items-center justify-between gap-3 rounded-lg border bg-background px-3 py-2"
              >
                <div className="min-w-0">
                  <div className="truncate text-sm font-medium">
                    {tool.name}{" "}
                    <span className={tool.ok ? "text-success" : "text-warning"}>
                      ({tool.ok
                        ? t("settings.cliProxyConfig.ok")
                        : t("settings.cliProxyConfig.needsFix")})
                    </span>
                  </div>
                </div>

                <Button
                  size="sm"
                  disabled={tool.ok || busy}
                  onClick={() => void onApplyCliProxyConfig(tool.id)}
                >
                  {t("settings.cliProxyConfig.apply")}
                </Button>
              </div>
            );
          })}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <div className="flex items-start justify-between gap-3">
            <div className="space-y-1">
              <CardTitle className="flex items-center gap-2">
                <RefreshCw className="h-4 w-4" />
                {t("settings.cliTools.title")}
              </CardTitle>
              <CardDescription>{t("settings.cliTools.subtitle")}</CardDescription>
            </div>
            <Button
              size="sm"
              variant="outline"
              onClick={() => void onRefreshCliToolsStatus()}
              disabled={cliToolsLoading}
              className="gap-2"
            >
              <RefreshCw className={`h-4 w-4 ${cliToolsLoading ? "animate-spin" : ""}`} />
              {t("settings.cliTools.refresh")}
            </Button>
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          {!cliToolsStatus ? (
            <div className="text-sm text-muted-foreground">
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
              <div key={tool.id} className="flex items-center justify-between gap-4">
                <div className="min-w-0">
                  <div className="truncate text-sm font-medium">{tool.name}</div>
                  <div className="text-xs text-muted-foreground">
                    {installed
                      ? t("settings.cliTools.installedWithVersion", { version })
                      : t("settings.cliTools.notInstalled")}
                  </div>
                </div>
                <div className="flex items-center gap-3">
                  <Button
                    size="sm"
                    variant="outline"
                    disabled={busy}
                    onClick={() => void onInstallCliTool(tool.id)}
                  >
                    {installed ? t("settings.cliTools.update") : t("settings.cliTools.install")}
                  </Button>

                  <div className="flex items-center gap-2">
                    <div className="text-xs text-muted-foreground">
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
        </CardContent>
      </Card>
    </div>
  );
}
