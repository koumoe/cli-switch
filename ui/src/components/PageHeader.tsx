import type { ReactNode } from "react";

import { cn } from "@/lib/utils";

type PageHeaderProps = {
  title: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
  className?: string;
};

export function PageHeader({ title, description, actions, className }: PageHeaderProps) {
  return (
    <div
      className={cn(
        "animate-fade-up flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between",
        className
      )}
    >
      <div className="flex min-h-11 flex-col justify-center">
        <h1 className="font-heading text-lg font-semibold leading-6 tracking-tight">{title}</h1>
        <p
          className={cn(
            "mt-0.5 min-h-4 text-xs leading-4 text-fg-muted",
            !description && "invisible"
          )}
        >
          {description ?? "\u00A0"}
        </p>
      </div>

      {actions ? (
        <div className="flex min-h-11 flex-wrap items-center gap-2 sm:justify-end">
          {actions}
        </div>
      ) : null}
    </div>
  );
}
