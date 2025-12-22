import React, { useEffect, useState } from "react";
import { ArrowLeft, Database, ScrollText } from "lucide-react";
import { toast } from "sonner";
import { format } from "date-fns";
import type { DateRange } from "react-day-picker";
import {
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  DateRangePicker,
  dateRangeToMs,
  dateRangeToStrings,
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
import { useI18n } from "@/lib/i18n";
import { formatBytes } from "../lib";
import {
  clearLogs,
  clearRecords,
  getDbSize,
  getHealth,
  getLogsSize,
  type DbSize,
  type Health,
  type LogsSize,
} from "../api";

function joinPath(base: string, sub: string): string {
  const sep = base.includes("\\") ? "\\" : "/";
  if (base.endsWith(sep)) return `${base}${sub}`;
  return `${base}${sep}${sub}`;
}

export function SettingsMaintenancePage({ onBack }: { onBack: () => void }) {
  const { locale, t } = useI18n();
  const [health, setHealth] = useState<Health | null>(null);

  const [dbSize, setDbSize] = useState<DbSize | null>(null);
  const [dbSizeLoading, setDbSizeLoading] = useState(false);

  const [logsSize, setLogsSize] = useState<LogsSize | null>(null);
  const [logsSizeLoading, setLogsSizeLoading] = useState(false);

  const [recordsScope, setRecordsScope] = useState<"all" | "date_range">("all");
  const [recordsDateRange, setRecordsDateRange] = useState<DateRange | undefined>(undefined);
  const [recordsPromptOpen, setRecordsPromptOpen] = useState(false);
  const [recordsClearing, setRecordsClearing] = useState(false);

  const [logsScope, setLogsScope] = useState<"all" | "date_range">("all");
  const [logsDateRange, setLogsDateRange] = useState<DateRange | undefined>(undefined);
  const [logsPromptOpen, setLogsPromptOpen] = useState(false);
  const [logsClearing, setLogsClearing] = useState(false);

  async function refreshDbSize() {
    setDbSizeLoading(true);
    try {
      const next = await getDbSize();
      setDbSize(next);
    } catch (e) {
      toast.error(t("settings.storage.dbSizeFail"), { description: String(e) });
    } finally {
      setDbSizeLoading(false);
    }
  }

  async function refreshLogsSize() {
    setLogsSizeLoading(true);
    try {
      const next = await getLogsSize();
      setLogsSize(next);
    } catch (e) {
      toast.error(t("settings.maintenance.logsSizeFail"), { description: String(e) });
    } finally {
      setLogsSizeLoading(false);
    }
  }

  useEffect(() => {
    getHealth()
      .then(setHealth)
      .catch(() => setHealth({ status: "离线" }));

    void refreshDbSize();
    void refreshLogsSize();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const recordsDateStr = recordsDateRange?.from
    ? `${format(recordsDateRange.from, "yyyy-MM-dd")}${recordsDateRange.to ? ` ~ ${format(recordsDateRange.to, "yyyy-MM-dd")}` : ""}`
    : "-";

  const logsDateStr = logsDateRange?.from
    ? `${format(logsDateRange.from, "yyyy-MM-dd")}${logsDateRange.to ? ` ~ ${format(logsDateRange.to, "yyyy-MM-dd")}` : ""}`
    : "-";

  return (
    <div className="space-y-4 pb-4">
      <div className="flex items-center gap-2">
        <Button variant="outline" size="sm" onClick={onBack} className="gap-1.5">
          <ArrowLeft className="h-4 w-4" />
          {t("settings.maintenance.back")}
        </Button>
        <div className="min-w-0">
          <h1 className="text-lg font-semibold">{t("settings.maintenance.title")}</h1>
          <p className="text-muted-foreground text-xs mt-0.5">{t("settings.maintenance.subtitle")}</p>
        </div>
      </div>

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
            <Input value={health?.db_path ?? "-"} disabled className="font-mono text-sm" />
          </div>

          <div className="space-y-2">
            <label className="text-sm font-medium">{t("settings.storage.dbSize")}</label>
            <div className="flex gap-2">
              <Input value={dbSize ? formatBytes(dbSize.total_bytes) : "-"} disabled className="font-mono text-sm" />
              <Button variant="outline" onClick={() => void refreshDbSize()} disabled={dbSizeLoading}>
                {t("common.refresh")}
              </Button>
            </div>
          </div>

          <Dialog
            open={recordsPromptOpen}
            onOpenChange={(v) => {
              if (recordsClearing) return;
              setRecordsPromptOpen(v);
            }}
          >
            <DialogContent className="sm:max-w-[520px]">
              <DialogHeader>
                <DialogTitle>{t("settings.records.confirmTitle")}</DialogTitle>
                <DialogDescription>
                  {t(recordsScope === "date_range" ? "settings.records.confirmDateRange" : "settings.records.confirmAll", {
                    range: recordsDateStr,
                  })}
                </DialogDescription>
              </DialogHeader>
              <DialogFooter>
                <Button variant="outline" onClick={() => setRecordsPromptOpen(false)} disabled={recordsClearing}>
                  {t("common.cancel")}
                </Button>
                <Button
                  variant="destructive"
                  onClick={async () => {
                    setRecordsClearing(true);
                    try {
                      const msRange = recordsScope === "date_range" ? dateRangeToMs(recordsDateRange) : null;
                      if (recordsScope === "date_range" && !msRange) {
                        toast.error(t("settings.records.invalidDate"));
                        return;
                      }

                      const res = await clearRecords({
                        mode: recordsScope,
                        start_ms: msRange?.start_ms,
                        end_ms: msRange?.end_ms,
                      });
                      toast.success(t("settings.records.cleared"), {
                        description: t("settings.records.clearedDetail", {
                          usage: res.usage_events_deleted.toLocaleString(),
                          failures: res.channel_failures_deleted.toLocaleString(),
                        }),
                      });
                      setRecordsPromptOpen(false);
                      setRecordsDateRange(undefined);
                      await refreshDbSize();
                    } catch (e) {
                      toast.error(t("settings.records.clearFail"), { description: String(e) });
                    } finally {
                      setRecordsClearing(false);
                    }
                  }}
                  disabled={recordsClearing}
                >
                  {recordsClearing ? t("settings.records.clearing") : t("settings.records.clear")}
                </Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>

          <div className="flex items-center justify-between gap-4">
            <div className="flex-1 min-w-0">
              <div className="font-medium text-sm">{t("settings.maintenance.clearRecords")}</div>
              <div className="text-xs text-muted-foreground">{t("settings.maintenance.clearHint")}</div>
            </div>
            <div className="flex items-center gap-2 flex-shrink-0">
              <Select value={recordsScope} onValueChange={(v) => setRecordsScope(v as any)} disabled={recordsClearing}>
                <SelectTrigger className="w-[160px]">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">{t("settings.maintenance.scopeAll")}</SelectItem>
                  <SelectItem value="date_range">{t("settings.maintenance.scopeRange")}</SelectItem>
                </SelectContent>
              </Select>
              {recordsScope === "date_range" && (
                <DateRangePicker
                  value={recordsDateRange}
                  onChange={setRecordsDateRange}
                  placeholder={t("settings.records.selectRange")}
                  className="w-[280px]"
                  disabled={recordsClearing}
                  locale={locale}
                />
              )}
              <Button
                variant="destructive"
                size="sm"
                onClick={() => {
                  if (recordsScope === "date_range" && !recordsDateRange?.from) {
                    toast.error(t("settings.records.invalidDate"));
                    return;
                  }
                  setRecordsPromptOpen(true);
                }}
                disabled={recordsClearing || (recordsScope === "date_range" && !recordsDateRange?.from)}
              >
                {t("settings.records.clear")}
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <ScrollText className="h-4 w-4" />
            {t("settings.maintenance.logsTitle")}
          </CardTitle>
          <CardDescription>{t("settings.maintenance.logsSubtitle")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <label className="text-sm font-medium">{t("settings.logging.dir")}</label>
            <Input value={health?.data_dir ? joinPath(health.data_dir, "logs") : "-"} disabled className="font-mono text-sm" />
          </div>

          <div className="space-y-2">
            <label className="text-sm font-medium">{t("settings.maintenance.logsSize")}</label>
            <div className="flex gap-2">
              <Input value={logsSize ? formatBytes(logsSize.total_bytes) : "-"} disabled className="font-mono text-sm" />
              <Button variant="outline" onClick={() => void refreshLogsSize()} disabled={logsSizeLoading}>
                {t("common.refresh")}
              </Button>
            </div>
          </div>

          <Dialog
            open={logsPromptOpen}
            onOpenChange={(v) => {
              if (logsClearing) return;
              setLogsPromptOpen(v);
            }}
          >
            <DialogContent className="sm:max-w-[520px]">
              <DialogHeader>
                <DialogTitle>{t("settings.logging.confirmTitle")}</DialogTitle>
                <DialogDescription>
                  {t(logsScope === "date_range" ? "settings.logging.confirmDateRange" : "settings.logging.confirmAll", {
                    range: logsDateStr,
                  })}
                </DialogDescription>
              </DialogHeader>
              <DialogFooter>
                <Button variant="outline" onClick={() => setLogsPromptOpen(false)} disabled={logsClearing}>
                  {t("common.cancel")}
                </Button>
                <Button
                  variant="destructive"
                  onClick={async () => {
                    setLogsClearing(true);
                    try {
                      if (logsScope === "date_range") {
                        const r = dateRangeToStrings(logsDateRange);
                        if (!r) {
                          toast.error(t("settings.logging.invalidDate"));
                          return;
                        }
                        const res = await clearLogs({ mode: "date_range", start_date: r.start, end_date: r.end });
                        toast.success(t("settings.logging.cleared"), {
                          description: t("settings.logging.clearedDetail", {
                            deleted: res.deleted_files,
                            truncated: res.truncated_files,
                          }),
                        });
                      } else {
                        const res = await clearLogs({ mode: "all" });
                        toast.success(t("settings.logging.cleared"), {
                          description: t("settings.logging.clearedDetail", {
                            deleted: res.deleted_files,
                            truncated: res.truncated_files,
                          }),
                        });
                      }

                      setLogsPromptOpen(false);
                      setLogsDateRange(undefined);
                      await refreshLogsSize();
                    } catch (e) {
                      toast.error(t("settings.logging.clearFail"), { description: String(e) });
                    } finally {
                      setLogsClearing(false);
                    }
                  }}
                  disabled={logsClearing}
                >
                  {logsClearing ? t("settings.logging.clearing") : t("settings.logging.clear")}
                </Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>

          <div className="flex items-center justify-between gap-4">
            <div className="flex-1 min-w-0">
              <div className="font-medium text-sm">{t("settings.logging.clearLogs")}</div>
              <div className="text-xs text-muted-foreground">{t("settings.maintenance.clearHint")}</div>
            </div>
            <div className="flex items-center gap-2 flex-shrink-0">
              <Select value={logsScope} onValueChange={(v) => setLogsScope(v as any)} disabled={logsClearing}>
                <SelectTrigger className="w-[160px]">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">{t("settings.maintenance.scopeAll")}</SelectItem>
                  <SelectItem value="date_range">{t("settings.maintenance.scopeRange")}</SelectItem>
                </SelectContent>
              </Select>
              {logsScope === "date_range" && (
                <DateRangePicker
                  value={logsDateRange}
                  onChange={setLogsDateRange}
                  placeholder={t("settings.logging.selectRange")}
                  className="w-[280px]"
                  disabled={logsClearing}
                  locale={locale}
                />
              )}
              <Button
                variant="destructive"
                size="sm"
                onClick={() => {
                  if (logsScope === "date_range" && !logsDateRange?.from) {
                    toast.error(t("settings.logging.invalidDate"));
                    return;
                  }
                  setLogsPromptOpen(true);
                }}
                disabled={logsClearing || (logsScope === "date_range" && !logsDateRange?.from)}
              >
                {t("settings.logging.clear")}
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

