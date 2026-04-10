import { Database, FolderOpen } from "lucide-react";
import type { DateRange } from "react-day-picker";

import {
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Input,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui";
import { DateRangePicker } from "@/components/composed/date-range-picker";
import { useI18n } from "@/hooks/use-i18n";
import type { AppSettings, RecordsClearMode } from "@/types/api";
import type { Locale } from "@/types/locale";
import { LoggingSettingsCard } from "@/pages/settings/form-sections";

type DataSettingsProps = {
  settings: AppSettings | null;
  onSaved: (settings: AppSettings) => void;
  locale: Locale;
  dataDir: string;
  dbPath: string;
  dbSizeText: string;
  dbSizeLoading: boolean;
  onRefreshDbSize: () => void | Promise<void>;
  onOpenDataDir: () => void | Promise<void>;
  recordsType: Exclude<RecordsClearMode, "date_range">;
  onRecordsTypeChange: (value: Exclude<RecordsClearMode, "date_range">) => void;
  recordsTimeScope: "all" | "date_range";
  onRecordsTimeScopeChange: (value: "all" | "date_range") => void;
  recordsDateRange: DateRange | undefined;
  onRecordsDateRangeChange: (range: DateRange | undefined) => void;
  recordsPromptOpen: boolean;
  onRecordsPromptOpenChange: (open: boolean) => void;
  recordsClearing: boolean;
  onRequestClearRecords: () => void;
  onConfirmClearRecords: () => void | Promise<void>;
  recordsDateStr: string;
  logsSizeText: string;
  logsSizeLoading: boolean;
  onRefreshLogsSize: () => void | Promise<void>;
  logsScope: "all" | "date_range";
  onLogsScopeChange: (value: "all" | "date_range") => void;
  logsDateRange: DateRange | undefined;
  onLogsDateRangeChange: (range: DateRange | undefined) => void;
  logsPromptOpen: boolean;
  onLogsPromptOpenChange: (open: boolean) => void;
  logsClearing: boolean;
  onRequestClearLogs: () => void;
  onConfirmClearLogs: () => void | Promise<void>;
  logsDateStr: string;
};

export function DataSettings({
  settings,
  onSaved,
  locale,
  dataDir,
  dbPath,
  dbSizeText,
  dbSizeLoading,
  onRefreshDbSize,
  onOpenDataDir,
  recordsType,
  onRecordsTypeChange,
  recordsTimeScope,
  onRecordsTimeScopeChange,
  recordsDateRange,
  onRecordsDateRangeChange,
  recordsPromptOpen,
  onRecordsPromptOpenChange,
  recordsClearing,
  onRequestClearRecords,
  onConfirmClearRecords,
  recordsDateStr,
  logsSizeText,
  logsSizeLoading,
  onRefreshLogsSize,
  logsScope,
  onLogsScopeChange,
  logsDateRange,
  onLogsDateRangeChange,
  logsPromptOpen,
  onLogsPromptOpenChange,
  logsClearing,
  onRequestClearLogs,
  onConfirmClearLogs,
  logsDateStr,
}: DataSettingsProps) {
  const { t } = useI18n();

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <FolderOpen className="h-4 w-4" />
            {t("settings.storage.title")}
          </CardTitle>
          <CardDescription>{t("settings.storage.subtitle")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <label className="text-sm font-medium">{t("settings.storage.dataDir")}</label>
            <div className="flex gap-2">
              <Input value={dataDir} disabled className="font-mono text-sm" />
              <Button
                variant="outline"
                onClick={() => void onOpenDataDir()}
                disabled={!dataDir || dataDir === "-"}
              >
                {t("common.open")}
              </Button>
            </div>
            <p className="text-xs text-muted-foreground">{t("settings.storage.dataDirHint")}</p>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Database className="h-4 w-4" />
            {t("settings.maintenance.databaseTitle")}
          </CardTitle>
          <CardDescription>{t("settings.maintenance.databaseSubtitle")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <label className="text-sm font-medium">{t("settings.storage.dbFile")}</label>
            <Input value={dbPath} disabled className="font-mono text-sm" />
          </div>

          <div className="space-y-2">
            <label className="text-sm font-medium">{t("settings.storage.dbSize")}</label>
            <div className="flex gap-2">
              <Input value={dbSizeText} disabled className="font-mono text-sm" />
              <Button variant="outline" onClick={() => void onRefreshDbSize()} disabled={dbSizeLoading}>
                {t("common.refresh")}
              </Button>
            </div>
          </div>

          <Dialog open={recordsPromptOpen} onOpenChange={onRecordsPromptOpenChange}>
            <DialogContent className="sm:max-w-[520px]">
              <DialogHeader>
                <DialogTitle>{t("settings.records.confirmTitle")}</DialogTitle>
                <DialogDescription>
                  {t(
                    recordsTimeScope === "date_range"
                      ? recordsType === "errors"
                        ? "settings.records.confirmDateRangeErrors"
                        : "settings.records.confirmDateRange"
                      : recordsType === "errors"
                        ? "settings.records.confirmErrors"
                        : "settings.records.confirmAll",
                    { range: recordsDateStr },
                  )}
                </DialogDescription>
              </DialogHeader>
              <DialogFooter>
                <Button
                  variant="outline"
                  onClick={() => onRecordsPromptOpenChange(false)}
                  disabled={recordsClearing}
                >
                  {t("common.cancel")}
                </Button>
                <Button variant="destructive" onClick={() => void onConfirmClearRecords()} disabled={recordsClearing}>
                  {recordsClearing ? t("settings.records.clearing") : t("settings.records.clear")}
                </Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>

          <div className="flex items-center justify-between gap-4">
            <div className="min-w-0 flex-1">
              <div className="text-sm font-medium">{t("settings.maintenance.clearRecords")}</div>
              <div className="text-xs text-muted-foreground">{t("settings.maintenance.clearHint")}</div>
            </div>
            <div className="flex shrink-0 items-center gap-2">
              <Select
                value={recordsType}
                onValueChange={(value) =>
                  onRecordsTypeChange(value as Exclude<RecordsClearMode, "date_range">)
                }
                disabled={recordsClearing}
              >
                <SelectTrigger className="w-[140px]">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">{t("settings.maintenance.typeAll")}</SelectItem>
                  <SelectItem value="errors">{t("settings.maintenance.typeErrors")}</SelectItem>
                </SelectContent>
              </Select>
              <Select
                value={recordsTimeScope}
                onValueChange={(value) => onRecordsTimeScopeChange(value as "all" | "date_range")}
                disabled={recordsClearing}
              >
                <SelectTrigger className="w-[120px]">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">{t("settings.maintenance.timeAll")}</SelectItem>
                  <SelectItem value="date_range">{t("settings.maintenance.timeRange")}</SelectItem>
                </SelectContent>
              </Select>
              {recordsTimeScope === "date_range" ? (
                <DateRangePicker
                  value={recordsDateRange}
                  onChange={onRecordsDateRangeChange}
                  placeholder={t("settings.records.selectRange")}
                  className="w-[260px]"
                  disabled={recordsClearing}
                  locale={locale}
                />
              ) : null}
              <Button variant="destructive" size="sm" onClick={onRequestClearRecords} disabled={recordsClearing}>
                {t("settings.records.clear")}
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      <LoggingSettingsCard
        settings={settings}
        dataDir={dataDir ? `${dataDir}${dataDir.endsWith("/") || dataDir.endsWith("\\") ? "" : dataDir.includes("\\") ? "\\" : "/"}logs` : "-"}
        logsSizeText={logsSizeText}
        logsSizeLoading={logsSizeLoading}
        onRefreshLogsSize={onRefreshLogsSize}
        onSaved={onSaved}
      >
        <div className="space-y-4">
          <Dialog open={logsPromptOpen} onOpenChange={onLogsPromptOpenChange}>
            <DialogContent className="sm:max-w-[520px]">
              <DialogHeader>
                <DialogTitle>{t("settings.logging.confirmTitle")}</DialogTitle>
                <DialogDescription>
                  {t(
                    logsScope === "date_range"
                      ? "settings.logging.confirmDateRange"
                      : "settings.logging.confirmAll",
                    { range: logsDateStr },
                  )}
                </DialogDescription>
              </DialogHeader>
              <DialogFooter>
                <Button
                  variant="outline"
                  onClick={() => onLogsPromptOpenChange(false)}
                  disabled={logsClearing}
                >
                  {t("common.cancel")}
                </Button>
                <Button variant="destructive" onClick={() => void onConfirmClearLogs()} disabled={logsClearing}>
                  {logsClearing ? t("settings.logging.clearing") : t("settings.logging.clear")}
                </Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>

          <div className="flex items-center justify-between gap-4">
            <div className="min-w-0 flex-1">
              <div className="text-sm font-medium">{t("settings.logging.clearLogs")}</div>
              <div className="text-xs text-muted-foreground">{t("settings.maintenance.clearHint")}</div>
            </div>
            <div className="flex shrink-0 items-center gap-2">
              <Select
                value={logsScope}
                onValueChange={(value) => onLogsScopeChange(value as "all" | "date_range")}
                disabled={logsClearing}
              >
                <SelectTrigger className="w-[140px]">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">{t("settings.maintenance.timeAll")}</SelectItem>
                  <SelectItem value="date_range">{t("settings.maintenance.timeRange")}</SelectItem>
                </SelectContent>
              </Select>
              {logsScope === "date_range" ? (
                <DateRangePicker
                  value={logsDateRange}
                  onChange={onLogsDateRangeChange}
                  placeholder={t("settings.logging.selectRange")}
                  className="w-[260px]"
                  disabled={logsClearing}
                  locale={locale}
                />
              ) : null}
              <Button variant="destructive" size="sm" onClick={onRequestClearLogs} disabled={logsClearing}>
                {t("settings.logging.clear")}
              </Button>
            </div>
          </div>
        </div>
      </LoggingSettingsCard>
    </div>
  );
}
