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
        "flex h-[55px] shrink-0 items-center justify-between border-b border-slate-200 px-5 dark:border-slate-800",
        className
      )}
    >
      <div className="min-w-0">
        <h1 className="truncate text-base font-bold">{title}</h1>
        {description ? (
          <p className="mt-0.5 truncate text-[11px] text-slate-500 dark:text-slate-400">
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
