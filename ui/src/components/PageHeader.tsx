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
    <header
      className={cn(
        "flex h-[55px] shrink-0 items-center justify-between border-b border-border px-5",
        className
      )}
    >
      <div className="min-w-0">
        <h1 className="truncate text-base font-bold">{title}</h1>
        {description ? (
          <p className="mt-0.5 truncate text-[11px] text-muted-foreground">
            {description}
          </p>
        ) : null}
      </div>

      {actions ? (
        <div className="flex items-center gap-2">
          {actions}
        </div>
      ) : null}
    </header>
  );
}
