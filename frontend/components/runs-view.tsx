"use client";

import {
  Activity,
  AlertTriangle,
  CheckCircle2,
  ChevronDown,
  Clock3,
  Database,
  FileWarning,
  Loader2,
  Play,
  RefreshCw,
  Search,
  Sparkles,
  Zap,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { toast } from "sonner";

import { EmptyState } from "@/components/empty-state";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { ApiError, api } from "@/lib/api";
import { phaseLabel, useI18n, type TranslationKey } from "@/lib/i18n";
import type { SearchRun } from "@/lib/types";
import {
  cn,
  formatDateTime,
  formatDuration,
  formatRelative,
} from "@/lib/utils";

function statusVariant(status: SearchRun["status"]) {
  if (status === "completed") return "success" as const;
  if (status === "failed") return "danger" as const;
  return "warning" as const;
}

export function RunsView() {
  const { t, lang } = useI18n();
  const [runs, setRuns] = useState<SearchRun[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const initialized = useRef(false);
  const lastError = useRef<string | null>(null);

  const load = useCallback(
    async (signal?: AbortSignal) => {
      try {
        const data = await api.listRuns(30, signal);
        if (signal?.aborted) return;
        setRuns(data);
        setError(null);
        lastError.current = null;
        if (!initialized.current && data.length > 0) {
          initialized.current = true;
          setExpanded(new Set([data[0].id]));
        }
      } catch (err) {
        if (err instanceof DOMException && err.name === "AbortError") return;
        if (signal?.aborted) return;
        const message =
          err instanceof ApiError ? err.message : t("error.loadBody");
        setError(message);
        if (lastError.current !== message) {
          lastError.current = message;
          toast.error(t("error.loadTitle"), { description: message });
        }
      } finally {
        if (!signal?.aborted) setLoading(false);
      }
    },
    [t],
  );

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    void load(controller.signal);
    return () => controller.abort();
  }, [load, refreshKey]);

  useEffect(() => {
    const refresh = () => setRefreshKey((key) => key + 1);
    window.addEventListener("hunter:run-started", refresh);
    window.addEventListener("hunter:run-finished", refresh);
    return () => {
      window.removeEventListener("hunter:run-started", refresh);
      window.removeEventListener("hunter:run-finished", refresh);
    };
  }, []);

  const anyRunning = runs.some((run) => run.status === "running");

  useEffect(() => {
    if (!anyRunning) return;
    const controller = new AbortController();
    const interval = setInterval(() => {
      void load(controller.signal);
    }, 2200);
    return () => {
      clearInterval(interval);
      controller.abort();
    };
  }, [anyRunning, load]);

  const toggle = (id: string) => {
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  return (
    <section className="pt-10 sm:pt-12">
      <div className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <div className="flex items-center gap-2">
            <span className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10 text-primary">
              <Activity className="h-5 w-5" />
            </span>
            <div>
              <h1 className="font-display text-2xl font-extrabold tracking-tight sm:text-3xl">
                {t("runs.title")}
              </h1>
              <p className="mt-1 text-sm text-muted-foreground">{t("runs.subtitle")}</p>
            </div>
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          {anyRunning ? (
            <Badge variant="warning" className="gap-1.5">
              <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-amber-500" />
              {t("runs.autoRefresh")}
            </Badge>
          ) : null}
          <Button
            variant="outline"
            size="sm"
            onClick={() => setRefreshKey((key) => key + 1)}
            disabled={loading}
          >
            <RefreshCw className={cn("h-3.5 w-3.5", loading && "animate-spin")} />
            {t("runs.refresh")}
          </Button>
          <Button
            size="sm"
            onClick={() => window.dispatchEvent(new CustomEvent("hunter:start-run"))}
          >
            <Play className="h-3.5 w-3.5 fill-current" />
            {t("header.run")}
          </Button>
        </div>
      </div>

      <div className="mt-7 space-y-3">
        {loading && runs.length === 0 ? (
          Array.from({ length: 3 }).map((_, index) => (
            <Card key={index} className="p-5">
              <div className="flex items-center justify-between">
                <Skeleton className="h-5 w-40" />
                <Skeleton className="h-5 w-20 rounded-full" />
              </div>
              <Skeleton className="mt-4 h-2 w-full rounded-full" />
              <div className="mt-5 grid grid-cols-2 gap-3 sm:grid-cols-4">
                {Array.from({ length: 4 }).map((__, cell) => (
                  <Skeleton key={cell} className="h-14 rounded-xl" />
                ))}
              </div>
            </Card>
          ))
        ) : error ? (
          <EmptyState
            icon={<AlertTriangle className="h-6 w-6" />}
            tone="danger"
            title={t("error.loadTitle")}
            body={error}
            actionLabel={t("error.retry")}
            onAction={() => setRefreshKey((key) => key + 1)}
          />
        ) : runs.length === 0 ? (
          <EmptyState
            icon={<Search className="h-6 w-6" />}
            title={t("runs.emptyTitle")}
            body={t("runs.emptyBody")}
            actionLabel={t("empty.cta")}
            onAction={() => window.dispatchEvent(new CustomEvent("hunter:start-run"))}
          />
        ) : (
          runs.map((run) => {
            const open = expanded.has(run.id);
            const running = run.status === "running";
            return (
              <Card
                key={run.id}
                className={cn(
                  "overflow-hidden transition-shadow",
                  running && "ring-1 ring-primary/25",
                )}
              >
                <button
                  type="button"
                  onClick={() => toggle(run.id)}
                  className="flex w-full items-start gap-3 p-5 text-left"
                  aria-expanded={open}
                >
                  <span
                    className={cn(
                      "mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-xl",
                      running
                        ? "bg-primary/10 text-primary"
                        : run.status === "completed"
                          ? "bg-emerald-500/10 text-emerald-600 dark:text-emerald-400"
                          : "bg-destructive/10 text-destructive",
                    )}
                  >
                    {running ? (
                      <Loader2 className="h-4 w-4 animate-spin" />
                    ) : run.status === "completed" ? (
                      <CheckCircle2 className="h-4 w-4" />
                    ) : (
                      <AlertTriangle className="h-4 w-4" />
                    )}
                  </span>

                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <Badge variant={statusVariant(run.status)}>
                        {t(`status.${run.status}` as TranslationKey)}
                      </Badge>
                      <Badge variant="outline" className="gap-1 border-border/70 text-muted-foreground">
                        <Zap className="h-3 w-3" />
                        {run.trigger === "cron" ? t("runs.cron") : t("runs.manual")}
                      </Badge>
                      <span className="text-[11px] font-medium text-muted-foreground">
                        {formatDateTime(run.started_at, lang)} ·{" "}
                        {formatRelative(run.started_at, lang)}
                      </span>
                    </div>
                    <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-muted-foreground">
                      <span className="font-mono text-[10px] opacity-70">
                        #{run.id.slice(0, 8)}
                      </span>
                      <span className="inline-flex items-center gap-1">
                        <Clock3 className="h-3 w-3" />
                        {formatDuration(run.started_at, run.finished_at)}
                      </span>
                      <span>{run.queries_count} {t("runs.searchQueries").toLowerCase()}</span>
                      <span>
                        {phaseLabel(run.phase, t)}
                        {running ? "…" : ""}
                      </span>
                    </div>
                  </div>

                  <ChevronDown
                    className={cn(
                      "mt-1 h-4 w-4 shrink-0 text-muted-foreground transition-transform",
                      open && "rotate-180",
                    )}
                  />
                </button>

                <div className="px-5">
                  <div className="h-1.5 overflow-hidden rounded-full bg-muted">
                    <div
                      className={cn(
                        "h-full rounded-full transition-all duration-700",
                        running
                          ? "bg-gradient-to-r from-primary to-fuchsia-500"
                          : run.status === "failed"
                            ? "bg-destructive"
                            : "bg-emerald-500",
                      )}
                      style={{ width: `${run.progress}%` }}
                    />
                  </div>
                  <div className="mt-1.5 flex justify-between text-[10px] font-semibold text-muted-foreground">
                    <span>{t("runs.progress")}</span>
                    <span>{run.progress}%</span>
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-2.5 px-5 py-4 sm:grid-cols-4">
                  <RunStat label={t("runs.searchQueries")} value={run.queries_count} icon={<Search className="h-3.5 w-3.5" />} />
                  <RunStat label={t("runs.found")} value={run.found_count} icon={<Sparkles className="h-3.5 w-3.5" />} />
                  <RunStat label={t("runs.new")} value={run.new_count} icon={<Database className="h-3.5 w-3.5" />} accent />
                  <RunStat
                    label={t("runs.failedUrls")}
                    value={run.errors?.length ?? 0}
                    icon={<FileWarning className="h-3.5 w-3.5" />}
                    danger={(run.errors?.length ?? 0) > 0}
                  />
                </div>

                {open ? (
                  <div className="grid gap-4 border-t border-border/70 bg-muted/25 p-5 lg:grid-cols-[1.4fr_1fr]">
                    <div>
                      <h3 className="flex items-center gap-1.5 text-[10px] font-extrabold uppercase tracking-[0.14em] text-muted-foreground">
                        <Activity className="h-3 w-3" />
                        {t("runs.logs")}
                      </h3>
                      <div className="mt-2 max-h-80 space-y-2 overflow-y-auto pr-1">
                        {(run.logs ?? []).length === 0 ? (
                          <p className="text-xs text-muted-foreground">{t("runs.noLogs")}</p>
                        ) : (
                          [...(run.logs ?? [])].reverse().map((entry, index) => (
                            <div
                              key={`${entry.at}-${index}`}
                              className="flex items-start gap-2 rounded-lg border border-border/60 bg-background/70 px-2.5 py-2"
                            >
                              <span
                                className={cn(
                                  "mt-1 h-1.5 w-1.5 shrink-0 rounded-full",
                                  entry.level === "error"
                                    ? "bg-destructive"
                                    : entry.level === "warn"
                                      ? "bg-amber-500"
                                      : "bg-emerald-500",
                                )}
                              />
                              <div className="min-w-0">
                                <p className="text-[12px] leading-snug text-foreground/85">
                                  {entry.message}
                                </p>
                                <p className="mt-0.5 font-mono text-[9px] text-muted-foreground">
                                  {new Date(entry.at).toLocaleTimeString(
                                    lang === "zh" ? "zh-CN" : "en-US",
                                    { hour: "2-digit", minute: "2-digit", second: "2-digit" },
                                  )}
                                </p>
                              </div>
                            </div>
                          ))
                        )}
                      </div>
                    </div>

                    <div>
                      <h3 className="flex items-center gap-1.5 text-[10px] font-extrabold uppercase tracking-[0.14em] text-muted-foreground">
                        <FileWarning className="h-3 w-3" />
                        {t("runs.errors")}
                      </h3>
                      <div className="mt-2 max-h-80 space-y-2 overflow-y-auto pr-1">
                        {(run.errors ?? []).length === 0 ? (
                          <div className="flex items-center gap-2 rounded-lg border border-emerald-500/20 bg-emerald-500/5 px-3 py-2.5 text-xs text-emerald-700 dark:text-emerald-300">
                            <CheckCircle2 className="h-3.5 w-3.5" />
                            {t("runs.noErrors")}
                          </div>
                        ) : (
                          (run.errors ?? []).map((entry, index) => (
                            <div
                              key={`${entry.at}-${index}`}
                              className="rounded-lg border border-destructive/20 bg-destructive/5 px-3 py-2.5"
                            >
                              <p className="text-[12px] leading-snug text-destructive">
                                {entry.message}
                              </p>
                              <p className="mt-1 font-mono text-[9px] text-muted-foreground">
                                {formatDateTime(entry.at, lang)}
                              </p>
                            </div>
                          ))
                        )}
                      </div>
                    </div>
                  </div>
                ) : null}
              </Card>
            );
          })
        )}
      </div>

      {runs.length > 0 ? (
        <p className="mt-5 text-center text-[11px] text-muted-foreground">
          {t("runs.viewLatest")}
        </p>
      ) : null}
    </section>
  );
}

function RunStat({
  label,
  value,
  icon,
  accent,
  danger,
}: {
  label: string;
  value: number;
  icon: ReactNode;
  accent?: boolean;
  danger?: boolean;
}) {
  return (
    <div className="rounded-xl border border-border/60 bg-background/60 px-3 py-2.5">
      <div className="flex items-center gap-1.5 text-[9px] font-bold uppercase tracking-[0.12em] text-muted-foreground">
        {icon}
        {label}
      </div>
      <p
        className={cn(
          "mt-1 text-lg font-extrabold tabular-nums",
          accent && "text-primary",
          danger && "text-destructive",
        )}
      >
        {value}
      </p>
    </div>
  );
}
