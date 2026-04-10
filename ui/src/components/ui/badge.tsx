import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "@/lib/utils"

const badgeVariants = cva(
  "inline-flex items-center whitespace-nowrap rounded-md px-2 py-0.5 text-[10px] font-semibold transition-colors",
  {
    variants: {
      variant: {
        default: "border-transparent bg-slate-100 text-slate-700 dark:bg-slate-800 dark:text-slate-200",
        secondary: "border-transparent bg-slate-100 text-slate-700 dark:bg-slate-800 dark:text-slate-200",
        destructive: "border-transparent bg-red-500/10 text-red-500 dark:text-red-400",
        success: "border-transparent bg-emerald-500/10 text-emerald-600 dark:text-emerald-400",
        warning: "border-transparent bg-amber-500/10 text-amber-600 dark:text-amber-400",
        outline: "border border-slate-200 bg-white text-slate-700 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-200",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  }
)

export interface BadgeProps
  extends React.HTMLAttributes<HTMLDivElement>,
    VariantProps<typeof badgeVariants> {}

function Badge({ className, variant, ...props }: BadgeProps) {
  return (
    <div className={cn(badgeVariants({ variant }), className)} {...props} />
  )
}

export { Badge, badgeVariants }
