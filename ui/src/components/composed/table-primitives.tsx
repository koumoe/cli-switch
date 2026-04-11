import type { ReactNode } from "react";

import { Button, type ButtonProps } from "@/components/ui";
import { cn } from "@/lib/utils";

type TableMetricStackProps = {
  primary: ReactNode;
  secondary?: ReactNode;
  className?: string;
  primaryClassName?: string;
  secondaryClassName?: string;
};

export function TableMetricStack({
  primary,
  secondary,
  className,
  primaryClassName,
  secondaryClassName,
}: TableMetricStackProps) {
  return (
    <div className={cn("space-y-0.5 text-center", className)}>
      <div className={primaryClassName}>{primary}</div>
      {secondary !== undefined ? (
        <div
          className={cn(
            "font-mono text-[10px] text-muted-foreground",
            secondaryClassName,
          )}
        >
          {secondary}
        </div>
      ) : null}
    </div>
  );
}

export function TableActionGroup({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div className={cn("flex items-center justify-center gap-1", className)}>
      {children}
    </div>
  );
}

export function TableIconButton({
  className,
  variant = "ghost",
  size = "icon",
  ...props
}: ButtonProps) {
  return (
    <Button
      className={cn("h-7 w-7 rounded-sm", className)}
      size={size}
      variant={variant}
      {...props}
    />
  );
}

export function TableTextActionButton({
  className,
  tone = "default",
  variant = "outline",
  size = "sm",
  ...props
}: ButtonProps & {
  tone?: "default" | "danger";
}) {
  return (
    <Button
      className={cn(
        "h-8 min-w-20 text-xs",
        tone === "danger"
          && "border-destructive/40 text-destructive hover:border-destructive hover:bg-destructive/10 hover:text-destructive",
        className,
      )}
      size={size}
      variant={variant}
      {...props}
    />
  );
}
