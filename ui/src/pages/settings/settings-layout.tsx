import type { ReactNode } from "react";

import { cn } from "@/lib/utils";

type SettingsSectionProps = {
  title: string;
  hint?: string;
  action?: ReactNode;
  first?: boolean;
  children?: ReactNode;
  className?: string;
};

type SettingsRowProps = {
  children: ReactNode;
  className?: string;
};

type SettingsFieldTextProps = {
  label: ReactNode;
  hint?: ReactNode;
  className?: string;
};

type SettingsFooterProps = {
  children: ReactNode;
  className?: string;
};

export function SettingsSection({
  title,
  hint,
  action,
  first = false,
  children,
  className,
}: SettingsSectionProps) {
  return (
    <section className={className}>
      <div
        className={cn(
          "border-t border-border px-5 pb-1 pt-2.5",
          first && "border-t-0",
        )}
      >
        <div className="flex items-center justify-between gap-3">
          <div className="text-[10px] font-bold uppercase tracking-[0.06em] text-muted-foreground">
            {title}
          </div>
          {action}
        </div>
        {hint ? (
          <div className="mt-0.5 text-[10.5px] leading-snug text-muted-foreground">
            {hint}
          </div>
        ) : null}
      </div>
      {children}
    </section>
  );
}

export function SettingsRow({ children, className }: SettingsRowProps) {
  return (
    <div
      className={cn(
        "flex min-h-[50px] items-center justify-between gap-4 border-t border-border px-5 py-3 transition-colors hover:bg-secondary/35",
        className,
      )}
    >
      {children}
    </div>
  );
}

export function SettingsFieldText({
  label,
  hint,
  className,
}: SettingsFieldTextProps) {
  return (
    <div className={cn("min-w-0 flex-1", className)}>
      <div className="text-[12.5px] font-semibold">{label}</div>
      {hint ? (
        <div className="mt-0.5 text-[10.5px] leading-snug text-muted-foreground">
          {hint}
        </div>
      ) : null}
    </div>
  );
}

export function SettingsFooter({ children, className }: SettingsFooterProps) {
  return (
    <div
      className={cn(
        "flex items-center justify-end gap-2 border-t border-border px-5 py-3",
        className,
      )}
    >
      {children}
    </div>
  );
}
