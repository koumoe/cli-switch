import type { ReactNode } from "react";

import { Card, CardContent, Skeleton } from "@/components/ui";
import { cn } from "@/lib/utils";

type MetricCardProps = {
  label: ReactNode;
  value: ReactNode;
  barColor: string;
  loading?: boolean;
  className?: string;
};

export function MetricCard({
  label,
  value,
  barColor,
  loading = false,
  className,
}: MetricCardProps) {
  return (
    <Card className={cn("relative overflow-hidden px-3.5 py-2.5", className)}>
      <span
        aria-hidden="true"
        className={cn("absolute inset-y-3 left-0 w-[3px] rounded-r", barColor)}
      />
      <CardContent className="space-y-1.5 p-0">
        <div className="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
          {label}
        </div>
        {loading ? (
          <Skeleton className="h-7 w-24" />
        ) : (
          <div className="text-xl font-extrabold tracking-tight">{value}</div>
        )}
      </CardContent>
    </Card>
  );
}
