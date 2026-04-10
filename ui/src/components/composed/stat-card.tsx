import type { ElementType, ReactNode } from "react";

import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  Skeleton,
} from "@/components/ui";

type StatCardProps = {
  icon: ElementType;
  label: ReactNode;
  value: ReactNode;
  loading?: boolean;
  className?: string;
};

export function StatCard({
  icon: Icon,
  label,
  value,
  loading = false,
  className,
}: StatCardProps) {
  return (
    <Card className={className}>
      <CardHeader className="px-3 pb-1.5 pt-3">
        <CardDescription className="flex items-center gap-1 text-xs">
          <Icon className="h-3 w-3" />
          {label}
        </CardDescription>
      </CardHeader>
      <CardContent className="px-3 pb-3">
        {loading ? (
          <Skeleton className="h-7 w-24" />
        ) : (
          <div className="text-xl font-bold">{value}</div>
        )}
      </CardContent>
    </Card>
  );
}
