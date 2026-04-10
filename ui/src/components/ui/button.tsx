import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { Slot } from "@radix-ui/react-slot"

import { cn } from "@/lib/utils"

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-1.5 whitespace-nowrap rounded-lg border text-xs font-semibold transition-colors outline-none select-none disabled:pointer-events-none disabled:opacity-50 focus-visible:ring-2 focus-visible:ring-blue-200 dark:focus-visible:ring-blue-900/60 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-3.5",
  {
    variants: {
      variant: {
        default:
          "border-blue-600 bg-blue-600 text-white hover:opacity-90 dark:border-blue-500 dark:bg-blue-500",
        outline:
          "border-slate-200 bg-white text-slate-900 hover:border-blue-600 hover:text-blue-600 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-100 dark:hover:border-blue-400 dark:hover:text-blue-400",
        secondary:
          "border-slate-200 bg-slate-100 text-slate-700 hover:border-blue-600 hover:text-blue-600 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:border-blue-400 dark:hover:text-blue-400",
        ghost:
          "border-transparent bg-transparent text-slate-500 hover:bg-blue-50 hover:text-blue-600 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-blue-400",
        destructive:
          "border-red-500 bg-red-500 text-white hover:opacity-90",
        link: "border-transparent p-0 text-blue-600 hover:underline dark:text-blue-400",
        success:
          "border-emerald-500 bg-emerald-500 text-white hover:opacity-90",
      },
      size: {
        default: "px-3 py-1.5",
        sm: "rounded-md px-2 py-1 text-[11px]",
        lg: "px-3.5 py-2",
        icon: "h-7 w-7 rounded-md p-0",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
)

function Button({
  className,
  variant = "default",
  size = "default",
  asChild = false,
  ...props
}: React.ComponentProps<"button"> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean
  }) {
  const Comp = asChild ? Slot : "button"

  return (
    <Comp
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  )
}

export { Button, buttonVariants }
