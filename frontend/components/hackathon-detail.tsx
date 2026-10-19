"use client";

import {
  ArrowUpRight,
  Bookmark,
  BookmarkCheck,
  Building2,
  CalendarDays,
  Check,
  Clock3,
  ExternalLink,
  Loader2,
  MapPin,
  Sparkles,
  Trophy,
  Users,
  Wifi,
  X,
} from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/dialog";
import { ApiError, api } from "@/lib/api";
import { pickTitle, useI18n } from "@/lib/i18n";
import type { Hackathon, HackathonStatus } from "@/lib/types";
import {
  cn,
  daysUntil,
  formatCny,
  formatDate,
  formatDateRange,
  formatRelative,
} from "@/lib/utils";

interface HackathonDetailProps {
  hackathon: Hackathon | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onUpdated: (hackathon: Hackathon) => void;
}

function statusLabel(status: HackathonStatus, t: ReturnType<typeof useI18n>["t"]) {
  switch (status) {
    case "saved":
      return t("card.saved");
    case "dismissed":
      return t("card.dismissed");
    default:
      return t("filters.new");
  }
}

export function HackathonDetail({
  hackathon,
  open,
  onOpenChange,
  onUpdated,
}: HackathonDetailProps) {
  const { t, lang } = useI18n();
  const [pending, setPending] = useState<"saved" | "dismissed" | null>(null);

  useEffect(() => {
    if (!open) setPending(null);
  }, [open]);

  if (!hackathon) return null;

  const title = pickTitle(hackathon, lang);
  const alternate =
    lang === "en" && hackathon.title_zh && hackathon.title_zh !== hackathon.title
      ? hackathon.title_zh
      : null;
  const days = daysUntil(hackathon.registration_deadline);
  const deadlineVariant =
    days === null ? "muted" : days < 0 || days <= 3 ? "danger" : days <= 7 ? "warning" : "success";
  const dateLabel = formatDateRange(hackathon.start_date, hackathon.end_date, lang);
  const prizeLabel =
    hackathon.prize_amount_cny !== null
      ? formatCny(hackathon.prize_amount_cny, lang)
      : hackathon.prize_summary;
  const registerUrl = hackathon.registration_url ?? hackathon.source_url;
  const isSaved = hackathon.status === "saved";
  const isDismissed = hackathon.status === "dismissed";

  const updateStatus = async (status: "saved" | "dismissed") => {
    setPending(status);
    try {
      const updated = await api.updateStatus(hackathon.id, status);
      toast.success(status === "saved" ? t("toast.saved") : t("toast.dismissed"));
      onUpdated(updated);
      onOpenChange(false);
    } catch (error) {
      toast.error(t("error.action"), {
        description:
          error instanceof ApiError ? error.message : "Please try again.",
      });
    } finally {
      setPending(null);
    }
  };

  const deadlineText = (() => {
    if (!hackathon.registration_deadline) return t("card.rolling");
    if (days === null) return formatDate(hackathon.registration_deadline, lang) ?? "—";
    if (days < 0) return t("card.closed");
    if (days === 0) return t("card.closesToday");
    return `${formatDate(hackathon.registration_deadline, lang)} · ${t("card.daysLeft", {
      count: days,
    })}`;
  })();

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="left-auto right-0 top-0 h-[100dvh] w-full max-w-xl translate-x-0 translate-y-0 gap-0 overflow-y-auto rounded-none border-y-0 border-r-0 p-0 shadow-2xl data-[state=open]:animate-slide-in-right data-[state=closed]:animate-out data-[state=closed]:slide-out-to-right sm:rounded-none"
        onOpenAutoFocus={(event) => event.preventDefault()}
      >
        <DialogDescription className="sr-only">
          {hackathon.description.slice(0, 160)}
        </DialogDescription>

        <div className="relative overflow-hidden bg-gradient-to-br from-slate-950 via-indigo-950 to-indigo-700 px-6 pb-7 pt-9 text-white">
          <div className="pointer-events-none absolute -right-20 -top-24 h-64 w-64 rounded-full bg-fuchsia-500/20 blur-3xl" />
          <div className="pointer-events-none absolute -bottom-28 -left-20 h-64 w-64 rounded-full bg-indigo-400/20 blur-3xl" />

          <div className="relative flex flex-wrap items-center gap-1.5">
            <span className="inline-flex items-center gap-1 rounded-full bg-white/12 px-2.5 py-0.5 text-[11px] font-bold uppercase tracking-wide text-white/90 ring-1 ring-inset ring-white/15">
              {statusLabel(hackathon.status, t)}
            </span>
            <span className="inline-flex items-center gap-1 rounded-full bg-white/12 px-2.5 py-0.5 text-[11px] font-semibold text-white/85 ring-1 ring-inset ring-white/15">
              <Clock3 className="h-3 w-3" />
              {deadlineText}
            </span>
            <span className="inline-flex items-center gap-1 rounded-full bg-white/12 px-2.5 py-0.5 text-[11px] font-semibold text-white/85 ring-1 ring-inset ring-white/15">
              {hackathon.is_online ? (
                <>
                  <Wifi className="h-3 w-3" /> {t("card.online")}
                </>
              ) : (
                <>
                  <MapPin className="h-3 w-3" /> {t("card.inPerson")}
                </>
              )}
            </span>
          </div>

          <DialogTitle className="relative mt-4 font-display text-2xl font-extrabold leading-tight tracking-tight text-white sm:text-[28px]">
            {title}
          </DialogTitle>
          {alternate ? (
            <p className="relative mt-1.5 text-[15px] leading-snug text-indigo-100/85">
              {alternate}
            </p>
          ) : null}
          <div className="relative mt-3 flex flex-wrap items-center gap-x-4 gap-y-1.5 text-xs text-indigo-100/75">
            <span className="inline-flex items-center gap-1.5">
              <Building2 className="h-3.5 w-3.5" />
              {hackathon.organizer ?? "—"}
            </span>
            {hackathon.location ? (
              <span className="inline-flex items-center gap-1.5">
                <MapPin className="h-3.5 w-3.5" />
                {hackathon.location}
              </span>
            ) : null}
          </div>
        </div>

        <div className="space-y-6 px-6 py-6">
          <section>
            <h3 className="text-[11px] font-extrabold uppercase tracking-[0.16em] text-muted-foreground">
              {t("detail.about")}
            </h3>
            <p className="mt-2 whitespace-pre-line text-sm leading-relaxed text-foreground/90">
              {hackathon.description}
            </p>
          </section>

          <section className="grid grid-cols-2 gap-2.5">
            <InfoTile
              icon={<CalendarDays className="h-4 w-4" />}
              label={t("detail.dates")}
              value={dateLabel ?? t("card.tba")}
            />
            <InfoTile
              icon={<Clock3 className="h-4 w-4" />}
              label={t("detail.deadline")}
              value={
                hackathon.registration_deadline
                  ? formatDate(hackathon.registration_deadline, lang) ?? "—"
                  : t("card.rolling")
              }
              accent={
                days !== null && days <= 7 && days >= 0
                  ? "text-rose-600 dark:text-rose-300"
                  : undefined
              }
            />
            <InfoTile
              icon={hackathon.is_online ? <Wifi className="h-4 w-4" /> : <MapPin className="h-4 w-4" />}
              label={t("detail.location")}
              value={
                hackathon.is_online
                  ? t("card.online")
                  : hackathon.location ?? t("card.locationTba")
              }
            />
            <InfoTile
              icon={<Sparkles className="h-4 w-4" />}
              label={t("detail.confidence")}
              value={`${Math.round(hackathon.confidence * 100)}%`}
            />
          </section>

          {prizeLabel ? (
            <section className="price-chip flex items-center gap-3 rounded-xl px-4 py-3.5">
              <span className="flex h-9 w-9 items-center justify-center rounded-lg bg-amber-400/25">
                <Trophy className="h-5 w-5" />
              </span>
              <div className="min-w-0">
                <p className="text-[10px] font-bold uppercase tracking-[0.14em] opacity-75">
                  {t("detail.prize")}
                </p>
                <p className="truncate text-base font-extrabold">{prizeLabel}</p>
              </div>
            </section>
          ) : null}

          <section>
            <h3 className="flex items-center gap-1.5 text-[11px] font-extrabold uppercase tracking-[0.16em] text-muted-foreground">
              {t("detail.tracks")}
            </h3>
            {hackathon.tracks_or_themes.length > 0 ? (
              <div className="mt-2.5 flex flex-wrap gap-1.5">
                {hackathon.tracks_or_themes.map((track) => (
                  <Badge key={track} variant="secondary" className="rounded-lg px-2.5 py-1">
                    {track}
                  </Badge>
                ))}
              </div>
            ) : (
              <p className="mt-2 text-sm text-muted-foreground">{t("detail.noTracks")}</p>
            )}
          </section>

          <section>
            <h3 className="flex items-center gap-1.5 text-[11px] font-extrabold uppercase tracking-[0.16em] text-muted-foreground">
              <Users className="h-3.5 w-3.5" />
              {t("detail.eligibility")}
            </h3>
            <p className="mt-2 rounded-xl border border-border/70 bg-muted/40 p-3.5 text-sm leading-relaxed text-foreground/85">
              {hackathon.eligibility ?? t("detail.noEligibility")}
            </p>
          </section>

          {hackathon.tags.length > 0 ? (
            <div className="flex flex-wrap gap-1.5">
              {hackathon.tags.map((tag) => (
                <span
                  key={tag}
                  className="rounded-md bg-primary/10 px-2 py-1 text-[11px] font-semibold text-primary"
                >
                  #{tag}
                </span>
              ))}
            </div>
          ) : null}

          <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-[11px] text-muted-foreground">
            <span>
              {t("detail.posted")} {formatRelative(hackathon.first_seen_at, lang)}
            </span>
            <span>
              {t("detail.lastSeen")} {formatRelative(hackathon.last_seen_at, lang)}
            </span>
            <a
              href={hackathon.source_url}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-1 font-semibold text-primary hover:underline"
            >
              <ExternalLink className="h-3 w-3" />
              {t("detail.source")}
            </a>
          </div>
        </div>

        <div className="sticky bottom-0 z-10 border-t border-border bg-background/95 p-4 backdrop-blur">
          <div className="flex flex-wrap items-center gap-2">
            <Button asChild className="h-10 flex-1 gap-1.5">
              <a href={registerUrl} target="_blank" rel="noopener noreferrer">
                {hackathon.registration_url ? t("detail.register") : t("card.source")}
                <ArrowUpRight className="h-4 w-4" />
              </a>
            </Button>
            <Button
              type="button"
              variant="outline"
              className="h-10 gap-1.5"
              disabled={pending !== null || isSaved || isDismissed}
              onClick={() => updateStatus("saved")}
            >
              {pending === "saved" ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : isSaved ? (
                <BookmarkCheck className="h-4 w-4" />
              ) : (
                <Bookmark className="h-4 w-4" />
              )}
              {isSaved ? t("card.saved") : t("detail.save")}
            </Button>
            <Button
              type="button"
              variant="ghost"
              className={cn(
                "h-10 gap-1.5 text-muted-foreground",
                isDismissed && "text-destructive",
              )}
              disabled={pending !== null || isDismissed}
              onClick={() => updateStatus("dismissed")}
            >
              {pending === "dismissed" ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : isDismissed ? (
                <Check className="h-4 w-4" />
              ) : (
                <X className="h-4 w-4" />
              )}
              {isDismissed ? t("card.dismissed") : t("detail.dismiss")}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

function InfoTile({
  icon,
  label,
  value,
  accent,
}: {
  icon: ReactNode;
  label: string;
  value: string;
  accent?: string;
}) {
  return (
    <div className="rounded-xl border border-border/70 bg-card p-3.5">
      <div className="flex items-center gap-1.5 text-[10px] font-bold uppercase tracking-[0.12em] text-muted-foreground">
        <span className="text-primary">{icon}</span>
        {label}
      </div>
      <p className={cn("mt-1.5 text-sm font-semibold leading-snug", accent)}>{value}</p>
    </div>
  );
}
