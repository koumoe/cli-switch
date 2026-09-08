import { Progress } from "@/components/ui";
import { ProtocolBadge } from "@/components/composed/protocol-badge";
import { resolveChannelIdentity } from "@/lib/channel-display";
import type { Channel, ChannelStats, Protocol } from "@/types/api";

import { formatNumber } from "../../lib";

type ChannelDistributionProps = {
  stats: ChannelStats[];
  channelsById: ReadonlyMap<string, Channel>;
  accountNames: Readonly<Record<string, string>>;
  protocolLabel: (protocol: Protocol) => string;
  view: "percent" | "usage";
};

export function ChannelDistribution({
  stats,
  channelsById,
  accountNames,
  protocolLabel,
  view,
}: ChannelDistributionProps) {
  const total = stats.reduce((sum, item) => sum + item.success, 0);
  if (total === 0) {
    return null;
  }

  const sorted = [...stats].sort((left, right) => right.success - left.success);

  return (
    <div className="space-y-3">
      {sorted.map((item) => {
        const identity = resolveChannelIdentity(channelsById.get(item.channel_id), accountNames, item.name);
        const percent = Math.round((item.success / total) * 100);
        return (
          <div key={item.channel_id} className="space-y-1">
            <div className="mb-1 flex items-center justify-between text-[11px]">
              <div className="min-w-0 flex items-center gap-1.5">
                <ProtocolBadge
                  protocol={item.protocol}
                  className="shrink-0 px-1.5 py-px text-[9px]"
                >
                  {protocolLabel(item.protocol)}
                </ProtocolBadge>
                <span className="truncate whitespace-nowrap" title={identity.label}>
                  <span className="font-medium text-foreground">{identity.accountName}</span>
                  <span className="text-muted-foreground"> · {identity.channelName}</span>
                </span>
              </div>
              <span className="ml-2 shrink-0 text-[10px] font-semibold text-muted-foreground">
                {view === "percent"
                  ? `${formatNumber(percent)}%`
                  : formatNumber(item.success)}
              </span>
            </div>
            <Progress
              value={percent}
              aria-label={identity.label}
              aria-valuenow={percent}
              indicatorClassName="bg-primary"
            />
          </div>
        );
      })}
    </div>
  );
}
