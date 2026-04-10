import { Badge, Progress } from "@/components/ui";
import type { ChannelStats, Protocol } from "@/types/api";

import {
  formatNumber,
  protocolBadgeClassName,
  protocolProgressClassName,
} from "../../lib";

type ChannelDistributionProps = {
  stats: ChannelStats[];
  protocolLabel: (protocol: Protocol) => string;
  view: "percent" | "usage";
};

export function ChannelDistribution({
  stats,
  protocolLabel,
  view,
}: ChannelDistributionProps) {
  const total = stats.reduce((sum, item) => sum + item.success, 0);
  if (total === 0) {
    return null;
  }

  const sorted = [...stats].sort((left, right) => right.success - left.success);

  return (
    <div className="space-y-2">
      {sorted.map((item) => {
        const percent = Math.round((item.success / total) * 100);
        return (
          <div key={item.channel_id} className="space-y-1">
            <div className="flex items-center justify-between text-xs">
              <div className="min-w-0 flex items-center gap-2 font-medium">
                <Badge
                  variant="outline"
                  className={`px-1 py-0 text-[10px] ${protocolBadgeClassName(item.protocol)}`}
                >
                  {protocolLabel(item.protocol)}
                </Badge>
                <span className="truncate">{item.name}</span>
              </div>
              <span className="ml-2 text-muted-foreground">
                {view === "percent" ? `${formatNumber(percent)}%` : formatNumber(item.success)}
              </span>
            </div>
            <Progress
              value={percent}
              className="h-2"
              indicatorClassName={protocolProgressClassName(item.protocol)}
            />
          </div>
        );
      })}
    </div>
  );
}
