"use client";

import { Sparkles } from "lucide-react";

import { useI18n } from "@/lib/i18n";

export function SiteFooter() {
  const { t } = useI18n();
  return (
    <footer className="border-t border-border/60 bg-background/40">
      <div className="mx-auto flex w-full max-w-7xl flex-col gap-3 px-4 py-7 text-xs text-muted-foreground sm:flex-row sm:items-center sm:justify-between sm:px-6 lg:px-8">
        <div className="flex items-center gap-2">
          <span className="flex h-6 w-6 items-center justify-center rounded-md bg-gradient-to-br from-indigo-500 to-fuchsia-500 text-white">
            <Sparkles className="h-3 w-3" />
          </span>
          <span className="font-semibold text-foreground">Hackathon Hunter</span>
          <span className="hidden sm:inline">· {t("footer.rights")}</span>
        </div>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
          <span>{t("footer.built")}</span>
          <span className="hidden h-3 w-px bg-border sm:block" />
          <span className="inline-flex items-center gap-1.5">
            <span className="h-1.5 w-1.5 rounded-full bg-emerald-500" />
            Asia/Shanghai
          </span>
        </div>
      </div>
    </footer>
  );
}
