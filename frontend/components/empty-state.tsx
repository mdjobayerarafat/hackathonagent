import type { ReactNode } from "react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface EmptyStateProps {
  icon: ReactNode;
  title: string;
  body: string;
  actionLabel?: string;
  onAction?: () => void;
  secondary?: ReactNode;
  tone?: "default" | "danger";
  className?: string;
}

export function EmptyState({
  icon,
  title,
  body,
  actionLabel,
  onAction,
  secondary,
  tone = "default",
  className,
}: EmptyStateProps) {
  return (
    <div
      className={cn(
        "flex flex-col items-center justify-center rounded-2xl border border-dashed border-border/80 bg-card/50 px-6 py-16 text-center",
        className,
      )}
    >
      <div
        className={cn(
          "flex h-14 w-14 items-center justify-center rounded-2xl",
          tone === "danger"
            ? "bg-destructive/10 text-destructive"
            : "bg-primary/10 text-primary",
        )}
      >
        {icon}
      </div>
      <h3 className="mt-4 font-display text-lg font-extrabold tracking-tight">{title}</h3>
      <p className="mt-1.5 max-w-sm text-sm leading-relaxed text-muted-foreground">{body}</p>
      {actionLabel && onAction ? (
        <div className="mt-5 flex flex-wrap justify-center gap-2">
          <Button onClick={onAction}>{actionLabel}</Button>
          {secondary}
        </div>
      ) : secondary ? (
        <div className="mt-5">{secondary}</div>
      ) : null}
    </div>
  );
}
