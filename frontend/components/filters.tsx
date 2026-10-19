"use client";

import {
  ChevronDown,
  LayoutGrid,
  ListOrdered,
  Search,
  SlidersHorizontal,
  Timer,
  Wifi,
  X,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useI18n, type TranslationKey } from "@/lib/i18n";
import { cn } from "@/lib/utils";

export type ViewMode = "grid" | "timeline";

interface FiltersProps {
  search: string;
  onSearchChange: (value: string) => void;
  status: string;
  onStatusChange: (status: string) => void;
  sort: string;
  onSortChange: (sort: string) => void;
  onlineOnly: boolean;
  onOnlineChange: (value: boolean) => void;
  view: ViewMode;
  onViewChange: (view: ViewMode) => void;
  total: number;
}

const tabs: Array<{ value: string; key: TranslationKey }> = [
  { value: "new", key: "filters.new" },
  { value: "closing_soon", key: "filters.closing" },
  { value: "saved", key: "filters.saved" },
  { value: "dismissed", key: "filters.dismissed" },
  { value: "all", key: "filters.all" },
];

const sortOptions: Array<{ value: string; key: TranslationKey }> = [
  { value: "deadline_asc", key: "filters.sortDeadline" },
  { value: "newest", key: "filters.sortNewest" },
  { value: "prize_desc", key: "filters.sortPrize" },
  { value: "confidence_desc", key: "filters.sortConfidence" },
];

export function Filters({
  search,
  onSearchChange,
  status,
  onStatusChange,
  sort,
  onSortChange,
  onlineOnly,
  onOnlineChange,
  view,
  onViewChange,
  total,
}: FiltersProps) {
  const { t } = useI18n();

  return (
    <div className="space-y-4">
      <div className="flex flex-col gap-2.5 lg:flex-row lg:items-center">
        <div className="relative flex-1">
          <Search className="pointer-events-none absolute left-3.5 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            value={search}
            onChange={(event) => onSearchChange(event.target.value)}
            placeholder={t("filters.search")}
            className="h-11 rounded-xl border-border/80 bg-card pl-10 pr-9 shadow-sm"
            aria-label={t("filters.search")}
          />
          {search ? (
            <button
              type="button"
              onClick={() => onSearchChange("")}
              className="absolute right-3 top-1/2 -translate-y-1/2 rounded-md p-1 text-muted-foreground hover:bg-muted hover:text-foreground"
              aria-label={t("filters.clear")}
            >
              <X className="h-3.5 w-3.5" />
            </button>
          ) : null}
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            role="switch"
            aria-checked={onlineOnly}
            onClick={() => onOnlineChange(!onlineOnly)}
            className={cn(
              "inline-flex h-11 items-center gap-2 rounded-xl border px-3.5 text-xs font-semibold transition-colors",
              onlineOnly
                ? "border-primary/30 bg-primary/10 text-primary"
                : "border-border/80 bg-card text-muted-foreground hover:text-foreground",
            )}
          >
            <Wifi className="h-3.5 w-3.5" />
            {t("filters.onlineOnly")}
            <span
              className={cn(
                "relative ml-0.5 flex h-4 w-7 items-center rounded-full p-0.5 transition-colors",
                onlineOnly ? "bg-primary" : "bg-muted-foreground/25",
              )}
            >
              <span
                className={cn(
                  "h-3 w-3 rounded-full bg-white shadow-sm transition-transform",
                  onlineOnly && "translate-x-3",
                )}
              />
            </span>
          </button>

          <div className="relative inline-flex h-11 items-center">
            <ListOrdered className="pointer-events-none absolute left-3 h-3.5 w-3.5 text-muted-foreground" />
            <select
              value={sort}
              onChange={(event) => onSortChange(event.target.value)}
              aria-label={t("filters.sort")}
              className="h-11 cursor-pointer appearance-none rounded-xl border border-border/80 bg-card pl-8 pr-8 text-xs font-semibold text-foreground shadow-sm outline-none transition-colors hover:border-primary/30 focus:ring-2 focus:ring-ring"
            >
              {sortOptions.map((option) => (
                <option key={option.value} value={option.value}>
                  {t(option.key)}
                </option>
              ))}
            </select>
            <ChevronDown className="pointer-events-none absolute right-2.5 h-3.5 w-3.5 text-muted-foreground" />
          </div>

          <div className="flex h-11 items-center rounded-xl border border-border/80 bg-card p-1 shadow-sm">
            <button
              type="button"
              onClick={() => onViewChange("grid")}
              className={cn(
                "flex h-8 items-center gap-1.5 rounded-lg px-2.5 text-xs font-semibold transition-colors",
                view === "grid"
                  ? "bg-primary/10 text-primary"
                  : "text-muted-foreground hover:text-foreground",
              )}
              aria-label={t("view.grid")}
              title={t("view.grid")}
            >
              <LayoutGrid className="h-3.5 w-3.5" />
              <span className="hidden xl:inline">{t("view.grid")}</span>
            </button>
            <button
              type="button"
              onClick={() => onViewChange("timeline")}
              className={cn(
                "flex h-8 items-center gap-1.5 rounded-lg px-2.5 text-xs font-semibold transition-colors",
                view === "timeline"
                  ? "bg-primary/10 text-primary"
                  : "text-muted-foreground hover:text-foreground",
              )}
              aria-label={t("view.timeline")}
              title={t("view.timeline")}
            >
              <Timer className="h-3.5 w-3.5" />
              <span className="hidden xl:inline">{t("view.timeline")}</span>
            </button>
          </div>
        </div>
      </div>

      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border/70 pb-0.5">
        <div className="no-scrollbar flex items-center gap-0.5 overflow-x-auto">
          {tabs.map((tab) => {
            const active = status === tab.value;
            return (
              <button
                key={tab.value}
                type="button"
                onClick={() => onStatusChange(tab.value)}
                className={cn(
                  "relative whitespace-nowrap px-3 py-2.5 text-sm font-semibold transition-colors",
                  active
                    ? "text-foreground"
                    : "text-muted-foreground hover:text-foreground",
                )}
              >
                {t(tab.key)}
                {active ? (
                  <span className="absolute inset-x-2 -bottom-0.5 h-0.5 rounded-full bg-gradient-to-r from-indigo-500 to-fuchsia-500" />
                ) : null}
              </button>
            );
          })}
        </div>

        <div className="flex items-center gap-2 pb-2 pl-1 text-xs font-medium text-muted-foreground">
          <SlidersHorizontal className="h-3.5 w-3.5" />
          {t("filters.results", { count: total })}
        </div>
      </div>
    </div>
  );
}

export function FilterResetButton({ onClick }: { onClick: () => void }) {
  const { t } = useI18n();
  return (
    <Button variant="outline" onClick={onClick}>
      {t("filters.clear")}
    </Button>
  );
}
