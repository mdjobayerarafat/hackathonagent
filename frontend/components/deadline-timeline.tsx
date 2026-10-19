"use client";

import {
  ArrowUpRight,
  CalendarClock,
  ChevronRight,
  Clock3,
  MapPin,
  Trophy,
  Wifi,
} from "lucide-react";
import { useMemo } from "react";

import { Badge } from "@/components/ui/badge";
import { pickTitle, useI18n } from "@/lib/i18n";
import type { Hackathon } from "@/lib/types";
import { cn, daysUntil, formatCny, formatDate } from "@/lib/utils";

interface DeadlineTimelineProps {
  items: Hackathon[];
  onOpen: (hackathon: Hackathon) => void;
}

export function DeadlineTimeline({ items, onOpen }: DeadlineTimelineProps) {
  const { t, lang } = useI18n();

  const groups = useMemo(() => {
    const sorted = [...items].sort((a, b) => {
      if (!a.registration_deadline && !b.registration_deadline) return 0;
      if (!a.registration_deadline) return 1;
      if (!b.registration_deadline) return -1;
      return a.registration_deadline.localeCompare(b.registration_deadline);
    });

    const map = new Map<string, { label: string; items: Hackathon[] }>();
    for (const item of sorted) {
      const key = item.registration_deadline?.slice(0, 7) ?? "none";
      if (!map.has(key)) {
        const label =
          key === "none"
            ? t("timeline.noDeadline")
            : new Intl.DateTimeFormat(lang === "zh" ? "zh-CN" : "en-US", {
                month: "long",
                year: "numeric",
              }).format(new Date(`${key}-01T00:00:00`));
        map.set(key, { label, items: [] });
      }
      map.get(key)!.items.push(item);
    }
    return Array.from(map.entries()).map(([key, value]) => ({ key, ...value }));
  }, [items, lang, t]);

  return (
    <div>
      <div className="mb-6 flex flex-wrap items-end justify-between gap-3">
        <div>
          <h2 className="flex items-center gap-2 font-display text-xl font-extrabold tracking-tight">
            <CalendarClock className="h-5 w-5 text-primary" />
            {t("timeline.title")}
          </h2>
          <p className="mt-1 text-sm text-muted-foreground">{t("timeline.subtitle")}</p>
        </div>
        <Badge variant="secondary">{t("timeline.count", { count: items.length })}</Badge>
      </div>

      <div className="relative">
        <div className="absolute bottom-4 left-[15px] top-2 w-px bg-gradient-to-b from-primary/50 via-border to-transparent sm:left-[19px]" />
        <div className="space-y-8">
          {groups.map((group) => (
            <section key={group.key}>
              <div className="relative mb-3 flex items-center gap-3 pl-9 sm:pl-11">
                <span className="absolute left-[9px] top-1/2 h-3.5 w-3.5 -translate-y-1/2 rounded-full border-[3px] border-background bg-primary sm:left-[13px]" />
                <h3 className="text-sm font-extrabold uppercase tracking-[0.14em] text-foreground">
                  {group.label}
                </h3>
                <span className="text-xs text-muted-foreground">
                  {group.items.length}
                </span>
              </div>

              <div className="space-y-2.5 pl-9 sm:pl-11">
                {group.items.map((item, index) => {
                  const days = daysUntil(item.registration_deadline);
                  const urgency =
                    days === null
                      ? "muted"
                      : days <= 3
                        ? "danger"
                        : days <= 7
                          ? "warning"
                          : "success";
                  const title = pickTitle(item, lang);
                  return (
                    <button
                      key={item.id}
                      type="button"
                      onClick={() => onOpen(item)}
                      className="group flex w-full animate-slide-up items-center gap-3 rounded-xl border border-border/70 bg-card/80 p-3 text-left shadow-sm transition-all hover:-translate-y-0.5 hover:border-primary/30 hover:shadow-md sm:gap-4 sm:p-4"
                      style={{ animationDelay: `${Math.min(index, 8) * 30}ms` }}
                    >
                      <div
                        className={cn(
                          "flex h-14 w-14 shrink-0 flex-col items-center justify-center rounded-xl border text-center",
                          urgency === "danger" &&
                            "border-rose-500/25 bg-rose-500/10 text-rose-600 dark:text-rose-300",
                          urgency === "warning" &&
                            "border-amber-500/25 bg-amber-500/10 text-amber-700 dark:text-amber-300",
                          urgency === "success" &&
                            "border-emerald-500/25 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300",
                          urgency === "muted" &&
                            "border-border bg-muted/60 text-muted-foreground",
                        )}
                      >
                        {item.registration_deadline ? (
                          <>
                            <span className="text-lg font-extrabold leading-none">
                              {new Date(
                                `${item.registration_deadline}T00:00:00`,
                              ).getDate()}
                            </span>
                            <span className="mt-1 text-[9px] font-bold uppercase tracking-wider">
                              {new Intl.DateTimeFormat(
                                lang === "zh" ? "zh-CN" : "en-US",
                                { month: "short" },
                              ).format(
                                new Date(`${item.registration_deadline}T00:00:00`),
                              )}
                            </span>
                          </>
                        ) : (
                          <Clock3 className="h-5 w-5" />
                        )}
                      </div>

                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2">
                          <h4 className="truncate text-sm font-bold group-hover:text-primary">
                            {title}
                          </h4>
                        </div>
                        <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-muted-foreground">
                          <span className="inline-flex items-center gap-1">
                            {item.is_online ? (
                              <>
                                <Wifi className="h-3 w-3" />
                                {t("card.online")}
                              </>
                            ) : (
                              <>
                                <MapPin className="h-3 w-3" />
                                {item.location ?? "—"}
                              </>
                            )}
                          </span>
                          <span>
                            {formatDate(item.registration_deadline, lang) ??
                              t("card.rolling")}
                          </span>
                          {item.prize_amount_cny !== null ? (
                            <span className="inline-flex items-center gap-1 font-semibold text-amber-700 dark:text-amber-300">
                              <Trophy className="h-3 w-3" />
                              {formatCny(item.prize_amount_cny, lang)}
                            </span>
                          ) : null}
                        </div>
                      </div>

                      <div className="flex shrink-0 items-center gap-2">
                        {days !== null ? (
                          <Badge variant={urgency} className="hidden sm:inline-flex">
                            {days < 0
                              ? t("card.closed")
                              : days === 0
                                ? t("card.closesToday")
                                : t("card.daysLeft", { count: days })}
                          </Badge>
                        ) : null}
                        <ChevronRight className="h-4 w-4 text-muted-foreground transition-transform group-hover:translate-x-0.5 group-hover:text-primary" />
                      </div>
                    </button>
                  );
                })}
              </div>
            </section>
          ))}
        </div>
      </div>

      <div className="mt-6 flex justify-center">
        <a
          href="/runs"
          className="inline-flex items-center gap-1.5 text-xs font-semibold text-muted-foreground hover:text-primary"
        >
          <ArrowUpRight className="h-3.5 w-3.5" />
          {t("nav.runs")}
        </a>
      </div>
    </div>
  );
}
