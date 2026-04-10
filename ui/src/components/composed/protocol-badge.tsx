import type { HTMLAttributes } from "react";

import { Badge } from "@/components/ui";
import { protocolBadgeClassName } from "@/lib";
import type { Protocol } from "@/types/api";
import { cn } from "@/lib/utils";

type ProtocolBadgeProps = HTMLAttributes<HTMLDivElement> & {
  protocol: Protocol;
};

export function ProtocolBadge({
  protocol,
  className,
  ...props
}: ProtocolBadgeProps) {
  return (
    <Badge
      variant="outline"
      className={cn(
        "inline-flex items-center rounded-md px-2 py-0.5 text-[10px] font-bold tracking-wide",
        protocolBadgeClassName(protocol),
        className,
      )}
      {...props}
    />
  );
}
