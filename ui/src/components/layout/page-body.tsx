import type { HTMLAttributes } from "react";

import { cn } from "@/lib/utils";

export function PageBody({
  className,
  ...props
}: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn("mx-auto w-full max-w-[1160px] px-4 pb-4 pt-3", className)}
      {...props}
    />
  );
}
