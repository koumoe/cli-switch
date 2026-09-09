import { useCallback, useEffect, useMemo, useState } from "react";
import { Activity, RefreshCw } from "lucide-react";
import { getActivities } from "@/api";
import { PageHeader } from "@/components/PageHeader";
import { PageBody } from "@/components/layout/page-body";
import { Badge, Button, Card, CardContent } from "@/components/ui";
import { useI18n } from "@/hooks/use-i18n";
import { humanizeApiError } from "@/lib/error";
import type { ActivityEntry, ActivitySnapshot } from "@/types/api";
import { formatDateTime } from "@/lib";

function activityTitle(entry: ActivityEntry, t: (key: string, vars?: Record<string, string | number>) => string): string {
  if (entry.title?.trim()) return entry.title.trim();
  if (entry.source === "codex" && entry.thread_id?.trim()) {
    return `Codex · ${entry.thread_id.trim().slice(0, 8)}`;
  }
  return t("activities.unassigned");
}

function statusTone(status: ActivityEntry["status"]): "default" | "secondary" | "destructive" | "outline" {
  if (status === "failed" || status === "cancelled") return "destructive";
  if (status === "running") return "default";
  if (status === "completed") return "secondary";
  return "outline";
}

export function ActivitiesPage() {
  const { t } = useI18n();
  const [snapshot, setSnapshot] = useState<ActivitySnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const load = useCallback(async () => {
    try {
      setError(null);
      setSnapshot(await getActivities());
    } catch (e) {
      setError(humanizeApiError(e, t));
    } finally {
      setLoading(false);
    }
  }, [t]);
  useEffect(() => {
    void load();
    const timer = window.setInterval(() => void load(), 2500);
    const onActivitiesChanged = () => void load();
    window.addEventListener("cliswitch-activities-changed", onActivitiesChanged);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener("cliswitch-activities-changed", onActivitiesChanged);
    };
  }, [load]);
  const entries = useMemo(() => snapshot?.entries ?? [], [snapshot]);
  return (
    <PageBody>
      <PageHeader
        title={t("activities.title")}
        description={t("activities.description")}
        actions={<Button variant="outline" size="sm" onClick={() => void load()} disabled={loading}><RefreshCw className="mr-1.5 h-3.5 w-3.5" />{t("common.refresh")}</Button>}
      />
      {error ? <div className="px-5 py-3 text-sm text-destructive">{error}</div> : null}
      {!loading && entries.length === 0 ? (
        <Card><CardContent className="flex flex-col items-center gap-2 py-12 text-center text-muted-foreground"><Activity className="h-7 w-7" /><div>{t("activities.empty")}</div></CardContent></Card>
      ) : (
        <div className="grid gap-3">
          {entries.map((entry) => {
            const title = activityTitle(entry, t);
            const statusText = t(`activities.status.${entry.status}`);
            const detail = [entry.source, entry.protocol, entry.model, entry.project].filter(Boolean).join(" · ");
            return <Card key={entry.id}><CardContent className="flex items-start justify-between gap-4 p-4"><div className="min-w-0"><div className="truncate text-sm font-semibold">{title}</div><div className="mt-1 truncate text-xs text-muted-foreground">{detail || t("activities.noDetails")}</div><div className="mt-1 text-[11px] text-muted-foreground">{formatDateTime(entry.updated_at_ms)}</div></div><Badge variant={statusTone(entry.status)}>{statusText}</Badge></CardContent></Card>;
          })}
          {snapshot && snapshot.omitted_running > 0 ? <div className="text-xs text-muted-foreground">{t("activities.omitted", { count: snapshot.omitted_running })}</div> : null}
        </div>
      )}
    </PageBody>
  );
}
