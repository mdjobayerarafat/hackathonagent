"use client";

import {
  AlertTriangle,
  ArrowRight,
  BrainCircuit,
  CalendarClock,
  FileText,
  Mail,
  Play,
  RefreshCw,
  Search,
  SearchX,
  ServerCog,
  Sparkles,
  Trophy,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { toast } from "sonner";

import { DeadlineTimeline } from "@/components/deadline-timeline";
import { EmptyState } from "@/components/empty-state";
import { Filters, type ViewMode } from "@/components/filters";
import { HackathonCard } from "@/components/hackathon-card";
import { HackathonDetail } from "@/components/hackathon-detail";
import { HackathonSkeletonGrid } from "@/components/hackathon-skeleton";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ApiError, api } from "@/lib/api";
import { useI18n } from "@/lib/i18n";
import type { Hackathon, HackathonFilters } from "@/lib/types";

const PAGE_SIZE = 24;

export function Dashboard() {
  const { t, lang } = useI18n();
  const [search, setSearch] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");
  const [status, setStatus] = useState("new");
  const [sort, setSort] = useState("deadline_asc");
  const [onlineOnly, setOnlineOnly] = useState(false);
  const [view, setView] = useState<ViewMode>("grid");
  const [items, setItems] = useState<Hackathon[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(0);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);
  const [selected, setSelected] = useState<Hackathon | null>(null);
  const [savingId, setSavingId] = useState<string | null>(null);
  const lastError = useRef<string | null>(null);

  useEffect(() => {
    const timer = setTimeout(() => setDebouncedSearch(search.trim()), 350);
    return () => clearTimeout(timer);
  }, [search]);

  const buildFilters = useCallback(
    (nextPage: number): HackathonFilters => ({
      status,
      sort,
      q: debouncedSearch || undefined,
      online: onlineOnly ? true : undefined,
      page: nextPage,
      page_size: PAGE_SIZE,
    }),
    [debouncedSearch, onlineOnly, sort, status],
  );

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setError(null);

    api
      .listHackathons(buildFilters(1), controller.signal)
      .then((response) => {
        if (controller.signal.aborted) return;
        setItems(response.items);
        setTotal(response.total);
        setPage(1);
        setTotalPages(response.total_pages);
        lastError.current = null;
      })
      .catch((err: unknown) => {
        if (err instanceof DOMException && err.name === "AbortError") return;
        if (controller.signal.aborted) return;
        const message =
          err instanceof ApiError ? err.message : t("error.loadBody");
        setError(message);
        if (lastError.current !== message) {
          lastError.current = message;
          toast.error(t("error.loadTitle"), { description: message });
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });

    return () => controller.abort();
  }, [buildFilters, refreshKey, t]);

  useEffect(() => {
    const refresh = () => setRefreshKey((key) => key + 1);
    window.addEventListener("hunter:run-finished", refresh);
    return () => window.removeEventListener("hunter:run-finished", refresh);
  }, []);

  const loadMore = async () => {
    if (loadingMore || page >= totalPages) return;
    setLoadingMore(true);
    try {
      const response = await api.listHackathons(buildFilters(page + 1));
      setItems((current) => {
        const ids = new Set(current.map((item) => item.id));
        return [...current, ...response.items.filter((item) => !ids.has(item.id))];
      });
      setPage(response.page);
      setTotalPages(response.total_pages);
      setTotal(response.total);
    } catch (err) {
      toast.error(t("error.loadTitle"), {
        description: err instanceof ApiError ? err.message : t("error.loadBody"),
      });
    } finally {
      setLoadingMore(false);
    }
  };

  const saveHackathon = async (hackathon: Hackathon) => {
    setSavingId(hackathon.id);
    try {
      await api.updateStatus(hackathon.id, "saved");
      toast.success(t("toast.saved"));
      if (selected?.id === hackathon.id) setSelected(null);
      setRefreshKey((key) => key + 1);
    } catch (err) {
      toast.error(t("error.action"), {
        description: err instanceof ApiError ? err.message : undefined,
      });
    } finally {
      setSavingId(null);
    }
  };

  const clearFilters = () => {
    setSearch("");
    setDebouncedSearch("");
    setStatus("new");
    setSort("deadline_asc");
    setOnlineOnly(false);
  };

  const filtered =
    Boolean(debouncedSearch) ||
    onlineOnly ||
    status !== "new";

  return (
    <>
      <section className="relative pt-10 sm:pt-14">
        <div className="hero-grid pointer-events-none absolute inset-x-0 -top-10 h-80" />
        <div className="relative grid items-center gap-8 lg:grid-cols-[1.35fr_1fr]">
          <div>
            <Badge
              variant="secondary"
              className="gap-1.5 rounded-full border border-primary/15 bg-primary/10 py-1 pl-1.5 pr-3 text-[11px] font-bold uppercase tracking-[0.14em] text-primary"
            >
              <span className="flex h-5 w-5 items-center justify-center rounded-full bg-primary/15">
                <Sparkles className="h-3 w-3" />
              </span>
              {t("hero.eyebrow")}
            </Badge>

            <h1 className="mt-5 max-w-2xl text-balance font-display text-[34px] font-extrabold leading-[1.08] tracking-[-0.03em] sm:text-5xl">
              {t("hero.title")}
            </h1>
            <p className="mt-4 max-w-xl text-balance text-[15px] leading-relaxed text-muted-foreground sm:text-base">
              {t("hero.subtitle")}
            </p>

            <div className="mt-6 flex flex-wrap items-center gap-2">
              <span className="inline-flex items-center gap-1.5 rounded-full border border-border/80 bg-card px-3 py-1.5 text-xs font-semibold text-muted-foreground shadow-sm">
                <CalendarClock className="h-3.5 w-3.5 text-primary" />
                {t("hero.chip")}
              </span>
              <span className="inline-flex items-center gap-1.5 rounded-full border border-border/80 bg-card px-3 py-1.5 text-xs font-semibold text-muted-foreground shadow-sm">
                <Trophy className="h-3.5 w-3.5 text-amber-500" />
                {t("filters.results", { count: loading ? "…" : total })}
              </span>
              <Button
                variant="ghost"
                size="sm"
                className="h-8 gap-1.5 px-2.5 text-xs font-semibold text-primary"
                onClick={() => {
                  window.dispatchEvent(new CustomEvent("hunter:start-run"));
                }}
              >
                <Play className="h-3 w-3 fill-current" />
                {t("header.run")}
              </Button>
            </div>
          </div>

          <div className="relative hidden overflow-hidden rounded-2xl border border-border/70 bg-card/70 p-5 shadow-soft backdrop-blur lg:block">
            <div className="pointer-events-none absolute -right-16 -top-16 h-40 w-40 rounded-full bg-primary/10 blur-3xl" />
            <div className="flex items-center justify-between">
              <h2 className="text-[11px] font-extrabold uppercase tracking-[0.16em] text-muted-foreground">
                {t("hero.how")}
              </h2>
              <span className="flex items-center gap-1 text-[10px] font-semibold text-emerald-600 dark:text-emerald-400">
                <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-emerald-500" />
                live
              </span>
            </div>
            <div className="mt-4 space-y-1">
              <PipelineRow
                icon={<Search className="h-4 w-4" />}
                title={t("pipeline.search")}
                description={t("pipeline.searchDesc")}
              />
              <PipelineConnector />
              <PipelineRow
                icon={<FileText className="h-4 w-4" />}
                title={t("pipeline.scrape")}
                description={t("pipeline.scrapeDesc")}
              />
              <PipelineConnector />
              <PipelineRow
                icon={<BrainCircuit className="h-4 w-4" />}
                title={t("pipeline.extract")}
                description={t("pipeline.extractDesc")}
              />
              <PipelineConnector />
              <PipelineRow
                icon={<Mail className="h-4 w-4" />}
                title={t("pipeline.email")}
                description={t("pipeline.emailDesc")}
              />
            </div>
            <div className="mt-4 flex items-center justify-between rounded-xl border border-border/60 bg-background/50 px-3 py-2 text-[10px] font-semibold text-muted-foreground">
              <span className="inline-flex items-center gap-1.5">
                <ServerCog className="h-3.5 w-3.5 text-primary" />
                PostgreSQL
              </span>
              <ArrowRight className="h-3 w-3" />
              <span>Next.js</span>
              <ArrowRight className="h-3 w-3" />
              <span>Resend</span>
            </div>
          </div>
        </div>
      </section>

      <section className="mt-10 sm:mt-12">
        <Filters
          search={search}
          onSearchChange={setSearch}
          status={status}
          onStatusChange={setStatus}
          sort={sort}
          onSortChange={setSort}
          onlineOnly={onlineOnly}
          onOnlineChange={setOnlineOnly}
          view={view}
          onViewChange={setView}
          total={total}
        />

        <div className="mt-6">
          {loading ? (
            <HackathonSkeletonGrid count={6} />
          ) : error ? (
            <EmptyState
              icon={<AlertTriangle className="h-6 w-6" />}
              tone="danger"
              title={t("error.loadTitle")}
              body={error}
              actionLabel={t("error.retry")}
              onAction={() => setRefreshKey((key) => key + 1)}
            />
          ) : items.length === 0 ? (
            filtered ? (
              <EmptyState
                icon={<SearchX className="h-6 w-6" />}
                title={t("empty.filteredTitle")}
                body={t("empty.filteredBody")}
                actionLabel={t("empty.filteredCta")}
                onAction={clearFilters}
              />
            ) : (
              <EmptyState
                icon={<Search className="h-6 w-6" />}
                title={t("empty.title")}
                body={t("empty.body")}
                actionLabel={t("empty.cta")}
                onAction={() => {
                  window.dispatchEvent(new CustomEvent("hunter:start-run"));
                }}
                secondary={
                  <Button variant="outline" asChild>
                    <a href="/runs">{t("nav.runs")}</a>
                  </Button>
                }
              />
            )
          ) : view === "grid" ? (
            <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
              {items.map((hackathon, index) => (
                <HackathonCard
                  key={hackathon.id}
                  hackathon={hackathon}
                  index={index}
                  onOpen={setSelected}
                  onSave={saveHackathon}
                  saving={savingId === hackathon.id}
                />
              ))}
            </div>
          ) : (
            <DeadlineTimeline items={items} onOpen={setSelected} />
          )}
        </div>

        {!loading && !error && items.length > 0 ? (
          <div className="mt-8 flex flex-col items-center gap-2">
            {page < totalPages ? (
              <Button
                variant="outline"
                onClick={loadMore}
                disabled={loadingMore}
                className="min-w-40"
              >
                {loadingMore ? (
                  <RefreshCw className="h-4 w-4 animate-spin" />
                ) : (
                  <ArrowRight className="h-4 w-4 rotate-90" />
                )}
                {t("filters.loadMore")}
              </Button>
            ) : (
              <p className="text-xs font-medium text-muted-foreground">
                {t("filters.noMore")}
              </p>
            )}
            <p className="text-[11px] text-muted-foreground/70">
              {items.length} / {total} · {lang.toUpperCase()}
            </p>
          </div>
        ) : null}
      </section>

      <HackathonDetail
        hackathon={selected}
        open={selected !== null}
        onOpenChange={(open) => {
          if (!open) setSelected(null);
        }}
        onUpdated={(updated) => {
          setSelected(null);
          setItems((current) =>
            current.map((item) => (item.id === updated.id ? updated : item)),
          );
          setRefreshKey((key) => key + 1);
        }}
      />
    </>
  );
}

function PipelineRow({
  icon,
  title,
  description,
}: {
  icon: ReactNode;
  title: string;
  description: string;
}) {
  return (
    <div className="flex items-center gap-3 rounded-xl px-1.5 py-1.5">
      <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
        {icon}
      </span>
      <div className="min-w-0">
        <p className="text-xs font-bold leading-tight">{title}</p>
        <p className="mt-0.5 truncate text-[10px] font-medium text-muted-foreground">
          {description}
        </p>
      </div>
    </div>
  );
}

function PipelineConnector() {
  return (
    <div className="ml-[22px] h-2.5 w-px bg-gradient-to-b from-border to-transparent" />
  );
}
