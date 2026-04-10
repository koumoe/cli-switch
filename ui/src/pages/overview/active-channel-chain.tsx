import { ArrowRight } from "lucide-react";

import { Badge } from "@/components/ui";
import type { AppSettings, Channel, Protocol } from "@/types/api";

import { protocolBadgeClassName } from "../../lib";

type ActiveChannelChainProps = {
  enabledByProtocol: Record<Protocol, Channel[]>;
  settings: AppSettings | null;
  protocolLabel: (protocol: Protocol) => string;
};

export function ActiveChannelChain({
  enabledByProtocol,
  settings,
  protocolLabel,
}: ActiveChannelChainProps) {
  return (
    <div className="space-y-3">
      {(["openai", "anthropic", "gemini"] as Protocol[])
        .filter((protocol) => enabledByProtocol[protocol].length > 0)
        .map((protocol) => {
          const list = enabledByProtocol[protocol];
          return (
            <div key={protocol} className="space-y-2">
              <div className="flex items-center gap-2">
                <Badge
                  variant="outline"
                  className={`px-2 py-0.5 text-[10px] ${protocolBadgeClassName(protocol)}`}
                >
                  {protocolLabel(protocol)}
                </Badge>
              </div>
              <div className="flex flex-wrap items-center gap-2">
                {list.map((channel, index) => (
                  <div key={channel.id} className="contents">
                    <div className="flex items-center gap-1.5 rounded border bg-card px-2 py-1">
                      <Badge variant="outline" className="px-1 py-0 text-[10px]">
                        {index + 1}
                      </Badge>
                      <span className="text-xs font-medium">
                        {settings?.channel_retry_enabled
                          ? `${channel.name} (${Math.max(1, channel.retry_times ?? 1)})`
                          : channel.name}
                      </span>
                    </div>
                    {index < list.length - 1 ? (
                      <ArrowRight className="h-3 w-3 text-muted-foreground" />
                    ) : null}
                  </div>
                ))}
              </div>
            </div>
          );
        })}
    </div>
  );
}
