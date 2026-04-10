import type { HTMLAttributes } from "react";

import claudeDarkIcon from "@/assets/protocol-icons/dark/claude.png";
import geminiDarkIcon from "@/assets/protocol-icons/dark/gemini.png";
import openaiDarkIcon from "@/assets/protocol-icons/dark/openai.png";
import claudeLightIcon from "@/assets/protocol-icons/light/claude.png";
import geminiLightIcon from "@/assets/protocol-icons/light/gemini.png";
import openaiLightIcon from "@/assets/protocol-icons/light/openai.png";
import { Badge } from "@/components/ui";
import { protocolBadgeClassName } from "@/lib";
import type { Protocol } from "@/types/api";
import { cn } from "@/lib/utils";

type ProtocolBadgeProps = HTMLAttributes<HTMLDivElement> & {
  protocol: Protocol;
};

function protocolIconSource(protocol: Protocol): {
  light: string;
  dark: string;
} {
  switch (protocol) {
    case "openai":
      return { light: openaiLightIcon, dark: openaiDarkIcon };
    case "anthropic":
      return { light: claudeLightIcon, dark: claudeDarkIcon };
    case "gemini":
      return { light: geminiLightIcon, dark: geminiDarkIcon };
  }
}

export function ProtocolBadge({
  protocol,
  className,
  ...props
}: ProtocolBadgeProps) {
  const icon = protocolIconSource(protocol);

  return (
    <Badge
      variant="outline"
      className={cn(
        "inline-flex items-center gap-1.5 rounded-md px-2 py-0.5 text-[10px] font-medium leading-none tracking-normal",
        protocolBadgeClassName(protocol),
        className,
      )}
      {...props}
    >
      <img
        aria-hidden="true"
        alt=""
        data-slot="protocol-icon"
        src={icon.light}
        className="h-3 w-3 shrink-0 object-contain dark:hidden"
      />
      <img
        aria-hidden="true"
        alt=""
        data-slot="protocol-icon"
        src={icon.dark}
        className="hidden h-3 w-3 shrink-0 object-contain dark:block"
      />
      {props.children}
    </Badge>
  );
}
