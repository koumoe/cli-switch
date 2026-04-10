import { FolderOpen, RefreshCw, Trash2 } from "lucide-react";
import type { DateRange } from "react-day-picker";

import {
  Button,
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
import { SettingsFieldText, SettingsRow, SettingsSection } from "./settings-layout";

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
    <div className="pb-4">
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

      <SettingsSection title={t("settings.storage.title")} first>
        <SettingsRow>
          <SettingsFieldText
            label={t("settings.storage.dataDir")}
            hint={t("settings.storage.dataDirHint")}
          />
          <div className="flex max-w-[380px] shrink-0 items-center gap-2">
            <span className="truncate text-[10px] font-mono text-slate-500 dark:text-slate-400" title={dataDir}>
              {dataDir}
            </span>
            <Button
              variant="outline"
              size="icon"
              onClick={() => void onOpenDataDir()}
              disabled={!dataDir || dataDir === "-"}
            >
              <FolderOpen className="h-3.5 w-3.5" />
            </Button>
          </div>
        </SettingsRow>

        <SettingsRow>
          <SettingsFieldText
            label={t("settings.storage.dbFile")}
            hint={t("settings.maintenance.databaseSubtitle")}
          />
          <span className="max-w-[380px] truncate text-[10px] font-mono text-slate-500 dark:text-slate-400" title={dbPath}>
            {dbPath}
          </span>
        </SettingsRow>

        <SettingsRow>
          <SettingsFieldText
            label={t("settings.storage.dbSize")}
            hint={t("settings.storage.subtitle")}
          />
          <div className="flex shrink-0 items-center gap-2">
            <span className="text-[11px] font-mono text-slate-500 dark:text-slate-400">{dbSizeText}</span>
            <Button variant="outline" size="icon" onClick={() => void onRefreshDbSize()} disabled={dbSizeLoading}>
              <RefreshCw className={`h-3.5 w-3.5 ${dbSizeLoading ? "animate-spin" : ""}`} />
            </Button>
          </div>
        </SettingsRow>

        <SettingsRow>
          <SettingsFieldText
            label={t("settings.maintenance.logsSize")}
            hint={t("settings.logging.subtitle")}
          />
          <div className="flex shrink-0 items-center gap-2">
            <span className="text-[11px] font-mono text-slate-500 dark:text-slate-400">{logsSizeText}</span>
            <Button
              variant="outline"
              size="icon"
              onClick={() => void onRefreshLogsSize()}
              disabled={logsSizeLoading}
            >
              <RefreshCw className={`h-3.5 w-3.5 ${logsSizeLoading ? "animate-spin" : ""}`} />
            </Button>
          </div>
        </SettingsRow>
      </SettingsSection>

      <LoggingSettingsCard settings={settings} onSaved={onSaved} />

      <SettingsSection title={t("settings.maintenance.cleanupTitle")}>
        <SettingsRow className="items-start">
          <SettingsFieldText
            label={t("settings.maintenance.clearRecords")}
            hint={t("settings.maintenance.clearHint")}
          />
          <div className="flex shrink-0 flex-wrap items-center justify-end gap-2">
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
              <Trash2 className="h-3.5 w-3.5" />
              {t("settings.records.clear")}
            </Button>
          </div>
        </SettingsRow>

        <SettingsRow className="items-start">
          <SettingsFieldText
            label={t("settings.logging.clearLogs")}
            hint={t("settings.maintenance.clearHint")}
          />
          <div className="flex shrink-0 flex-wrap items-center justify-end gap-2">
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
              <Trash2 className="h-3.5 w-3.5" />
              {t("settings.logging.clear")}
            </Button>
          </div>
        </SettingsRow>
      </SettingsSection>
    </div>
  );
}
