"use client";

import {
  ArrowUpRight,
  Bookmark,
  BookmarkCheck,
  Building2,
  CalendarDays,
  ChevronRight,
  Clock3,
  MapPin,
  Sparkles,
  Trophy,
  Wifi,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardFooter, CardHeader } from "@/components/ui/card";
import { pickTitle, useI18n } from "@/lib/i18n";
import type { Hackathon } from "@/lib/types";
import {
  cn,
  daysUntil,
  formatCny,
  formatDate,
  formatDateRange,
} from "@/lib/utils";

interface HackathonCardProps {
  hackathon: Hackathon;
  index: number;
  onOpen: (hackathon: Hackathon) => void;
  onSave: (hackathon: Hackathon) => void;
  saving?: boolean;
}

export function confidenceVariant(confidence: number) {
  if (confidence >= 0.85) return "success" as const;
  if (confidence >= 0.7) return "warning" as const;
  return "muted" as const;
}

export function HackathonCard({
  hackathon,
  index,
  onOpen,
  onSave,
  saving = false,
}: HackathonCardProps) {
  const { t, lang } = useI18n();
  const title = pickTitle(hackathon, lang);
  const alternateTitle =
    lang === "en" && hackathon.title_zh && hackathon.title_zh !== hackathon.title
      ? hackathon.title_zh
      : null;
  const days = daysUntil(hackathon.registration_deadline);
  const dateLabel = formatDateRange(
    hackathon.start_date,
    hackathon.end_date,
    lang,
  );
  const prizeAmount =
    hackathon.prize_amount_cny !== null
      ? formatCny(hackathon.prize_amount_cny, lang)
      : null;
  const prizeLabel = prizeAmount ?? hackathon.prize_summary;
  const registerUrl = hackathon.registration_url ?? hackathon.source_url;
  const extraTags = Math.max(0, hackathon.tags.length - 3);
  const isSaved = hackathon.status === "saved";
  const canSave = hackathon.status !== "dismissed";

  const deadline = (() => {
    if (days === null) {
      return {
        label: t("card.rolling"),
        variant: "muted" as const,
      };
    }
    if (days < 0) {
      return { label: t("card.closed"), variant: "danger" as const };
    }
    if (days === 0) {
      return { label: t("card.closesToday"), variant: "danger" as const };
    }
    if (days <= 7) {
      return {
        label: t("card.daysLeft", { count: days }),
        variant: "warning" as const,
      };
    }
    return {
      label: t("card.daysLeft", { count: days }),
      variant: "success" as const,
    };
  })();

  return (
    <Card
      className="group relative flex h-full animate-slide-up flex-col overflow-hidden border-border/70 bg-card/90 transition-all duration-300 hover:-translate-y-1 hover:border-primary/30 hover:shadow-lift"
      style={{ animationDelay: `${Math.min(index, 8) * 35}ms` }}
    >
      <div className="absolute inset-x-0 top-0 h-[3px] bg-gradient-to-r from-indigo-500 via-violet-500 to-fuchsia-500 opacity-0 transition-opacity duration-300 group-hover:opacity-100" />

      <CardHeader className="gap-0 p-5 pb-3">
        <div className="flex items-start justify-between gap-3">
          <div className="flex flex-wrap items-center gap-1.5">
            <Badge variant={deadline.variant} className="gap-1">
              <Clock3 className="h-3 w-3" />
              {deadline.label}
            </Badge>
            <Badge variant="outline" className="gap-1 border-border/70 text-muted-foreground">
              {hackathon.is_online ? (
                <>
                  <Wifi className="h-3 w-3" />
                  {t("card.online")}
                </>
              ) : (
                <>
                  <MapPin className="h-3 w-3" />
                  {hackathon.location ?? t("card.locationTba")}
                </>
              )}
            </Badge>
          </div>
          <span
            title={t("card.confidence")}
            className="shrink-0"
          >
            <Badge variant={confidenceVariant(hackathon.confidence)} className="gap-1 tabular-nums">
              <Sparkles className="h-3 w-3" />
              {Math.round(hackathon.confidence * 100)}%
            </Badge>
          </span>
        </div>

        <button
          type="button"
          onClick={() => onOpen(hackathon)}
          className="mt-3 text-left focus:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <h3 className="line-clamp-3 font-display text-[17px] font-extrabold leading-snug tracking-tight text-foreground transition-colors group-hover:text-primary">
            {title}
          </h3>
          {alternateTitle ? (
            <p className="mt-1 text-sm leading-snug text-muted-foreground">
              {alternateTitle}
            </p>
          ) : null}
        </button>

        <div className="mt-2 flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
          <Building2 className="h-3.5 w-3.5 shrink-0" />
          <span className="truncate">{hackathon.organizer ?? "—"}</span>
        </div>
      </CardHeader>

      <CardContent className="flex flex-1 flex-col gap-3 p-5 pt-0">
        <p className="line-clamp-2 text-[13px] leading-relaxed text-muted-foreground">
          {hackathon.description}
        </p>

        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs font-medium text-muted-foreground">
          <span className="inline-flex items-center gap-1.5">
            <CalendarDays className="h-3.5 w-3.5 text-indigo-500" />
            {dateLabel ?? t("card.tba")}
          </span>
          {hackathon.registration_deadline ? (
            <span className="inline-flex items-center gap-1.5">
              <Clock3 className="h-3.5 w-3.5 text-rose-500" />
              {t("card.deadline")}{" "}
              {formatDate(hackathon.registration_deadline, lang)}
            </span>
          ) : null}
        </div>

        {prizeLabel ? (
          <div className="price-chip inline-flex w-fit max-w-full items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-xs font-bold">
            <Trophy className="h-3.5 w-3.5 shrink-0" />
            <span className="truncate">{prizeLabel}</span>
            {prizeAmount && hackathon.prize_summary ? (
              <span className="sr-only">{hackathon.prize_summary}</span>
            ) : null}
          </div>
        ) : null}

        <div className="mt-auto flex flex-wrap items-center gap-1.5 pt-1">
          {hackathon.tags.slice(0, 3).map((tag) => (
            <span
              key={tag}
              className="rounded-md bg-muted px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground"
            >
              #{tag}
            </span>
          ))}
          {extraTags > 0 ? (
            <span className="text-[10px] font-semibold text-muted-foreground/70">
              +{extraTags}
            </span>
          ) : null}
        </div>
      </CardContent>

      <CardFooter className="gap-2 p-5 pt-0">
        <Button asChild className="h-10 flex-1 gap-1.5 rounded-lg">
          <a href={registerUrl} target="_blank" rel="noopener noreferrer">
            {hackathon.registration_url ? t("card.register") : t("card.source")}
            <ArrowUpRight className="h-4 w-4" />
          </a>
        </Button>
        <Button
          type="button"
          variant="outline"
          size="icon"
          className="h-10 w-10 shrink-0 rounded-lg"
          onClick={() => onOpen(hackathon)}
          aria-label={t("card.details")}
          title={t("card.details")}
        >
          <ChevronRight className="h-4 w-4" />
        </Button>
        {canSave ? (
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className={cn(
              "h-10 w-10 shrink-0 rounded-lg",
              isSaved && "text-primary",
            )}
            onClick={() => onSave(hackathon)}
            disabled={isSaved || saving}
            aria-label={isSaved ? t("card.saved") : t("card.save")}
            title={isSaved ? t("card.saved") : t("card.save")}
          >
            {isSaved ? (
              <BookmarkCheck className="h-4 w-4" />
            ) : (
              <Bookmark className="h-4 w-4" />
            )}
          </Button>
        ) : null}
      </CardFooter>
    </Card>
  );
}
