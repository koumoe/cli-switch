import { ArrowRight } from "lucide-react";

import { Badge } from "@/components/ui";
import { ProtocolBadge } from "@/components/composed/protocol-badge";
import type { AppSettings, Channel, Protocol } from "@/types/api";

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
              <div className="mb-2">
                <ProtocolBadge protocol={protocol}>
                  {protocolLabel(protocol)}
                </ProtocolBadge>
              </div>
              <div className="flex flex-wrap items-center gap-1.5">
                {list.map((channel, index) => (
                  <div key={channel.id} className="contents">
                    <div className="inline-flex items-center gap-1.5 rounded-lg border border-slate-200 bg-white px-2.5 py-1 text-[11.5px] font-medium dark:border-slate-800 dark:bg-slate-900">
                      <Badge className="flex h-[18px] w-[18px] items-center justify-center rounded bg-slate-100 px-0 text-[9px] font-extrabold text-slate-500 dark:bg-slate-800 dark:text-slate-400">
                        {index + 1}
                      </Badge>
                      <span>{channel.name}</span>
                      {settings?.channel_retry_enabled ? (
                        <span className="text-[10px] text-slate-500 dark:text-slate-400">
                          ({Math.max(1, channel.retry_times ?? 1)})
                        </span>
                      ) : null}
                    </div>
                    {index < list.length - 1 ? (
                      <ArrowRight className="h-3 w-3 text-slate-400/50 dark:text-slate-500/60" />
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
