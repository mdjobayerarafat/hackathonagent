"use client";

import { Loader2, Play, Sparkles, X } from "lucide-react";
import Link from "next/link";
import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { ApiError, api } from "@/lib/api";
import { phaseLabel, useI18n } from "@/lib/i18n";
import type { SearchRun } from "@/lib/types";
import { cn } from "@/lib/utils";

function isRunning(run: SearchRun | null) {
  return run?.status === "running";
}

export function RunAgentButton({ className }: { className?: string }) {
  const { t } = useI18n();
  const [starting, setStarting] = useState(false);
  const [run, setRun] = useState<SearchRun | null>(null);
  const [panelOpen, setPanelOpen] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const mounted = useRef(true);
  const polling = useRef(false);
  const tRef = useRef(t);
  const startRef = useRef<() => void>(() => {});

  useEffect(() => {
    tRef.current = t;
  }, [t]);

  const stopPolling = useCallback(() => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
    polling.current = false;
  }, []);

  const finish = useCallback(
    (finished: SearchRun) => {
      stopPolling();
      setRun(finished);
      if (finished.status === "completed") {
        toast.success(
          tRef.current("runToast.finished", {
            found: finished.found_count,
            new: finished.new_count,
          }),
        );
      } else {
        toast.error(tRef.current("runToast.failed"), {
          description:
            finished.errors?.[0]?.message ?? "Check the run log for details.",
        });
      }
      window.dispatchEvent(
        new CustomEvent("hunter:run-finished", { detail: finished }),
      );
    },
    [stopPolling],
  );

  const poll = useCallback(
    (runId: string) => {
      if (polling.current) return;
      polling.current = true;

      const tick = async () => {
        if (!mounted.current) return;
        try {
          const next = await api.getRun(runId);
          if (!mounted.current) return;
          setRun(next);
          if (next.status === "running") {
            timer.current = setTimeout(tick, 1800);
          } else {
            finish(next);
          }
        } catch {
          if (!mounted.current) return;
          timer.current = setTimeout(tick, 3500);
        }
      };

      void tick();
    },
    [finish],
  );

  useEffect(() => {
    mounted.current = true;
    api
      .listRuns(1)
      .then((runs) => {
        if (!mounted.current || polling.current) return;
        const latest = runs[0];
        if (latest && isRunning(latest)) {
          setRun(latest);
          setPanelOpen(true);
          poll(latest.id);
        }
      })
      .catch(() => {
        // The dashboard surfaces connection errors; no need to double-toast here.
      });

    return () => {
      mounted.current = false;
      stopPolling();
    };
  }, [poll, stopPolling]);

  const start = async () => {
    if (starting || isRunning(run)) return;
    setStarting(true);
    try {
      const accepted = await api.triggerRun();
      toast.success(t("runToast.started"));
      setPanelOpen(true);
      window.dispatchEvent(new CustomEvent("hunter:run-started"));
      const initial: SearchRun = {
        id: accepted.run_id,
        trigger: "manual",
        status: "running",
        phase: "starting",
        progress: 0,
        started_at: new Date().toISOString(),
        finished_at: null,
        queries_count: 0,
        found_count: 0,
        new_count: 0,
        errors: [],
        logs: [],
      };
      setRun(initial);
      poll(accepted.run_id);
    } catch (error) {
      if (error instanceof ApiError && error.status === 409) {
        toast.info(t("runToast.already"));
      } else {
        toast.error(t("error.run"), {
          description: error instanceof Error ? error.message : undefined,
        });
      }
    } finally {
      setStarting(false);
    }
  };

  startRef.current = () => {
    void start();
  };

  useEffect(() => {
    const handler = () => startRef.current();
    window.addEventListener("hunter:start-run", handler);
    return () => window.removeEventListener("hunter:start-run", handler);
  }, []);

  const running = isRunning(run);
  const progress = Math.max(0, Math.min(100, run?.progress ?? 0));

  return (
    <>
      <Button
        onClick={start}
        disabled={starting || running}
        className={cn("gap-2 shadow-sm", className)}
      >
        {starting || running ? (
          <Loader2 className="h-4 w-4 animate-spin" />
        ) : (
          <Play className="h-4 w-4 fill-current" />
        )}
        <span className="hidden sm:inline">
          {running ? t("status.running") : t("header.run")}
        </span>
      </Button>

      {panelOpen && run ? (
        <div className="fixed bottom-4 left-4 right-4 z-[60] w-auto animate-slide-up rounded-2xl border border-border bg-card/95 p-4 shadow-lift backdrop-blur sm:left-auto sm:right-5 sm:w-[380px]">
          <div className="flex items-start gap-3">
            <div className="mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-primary/10 text-primary">
              {running ? (
                <Sparkles className="h-4 w-4 animate-pulse" />
              ) : run.status === "completed" ? (
                <Sparkles className="h-4 w-4" />
              ) : (
                <X className="h-4 w-4" />
              )}
            </div>
            <div className="min-w-0 flex-1">
              <div className="flex items-start justify-between gap-2">
                <div>
                  <p className="text-sm font-bold leading-tight">
                    {t("runToast.progressTitle")}
                  </p>
                  <p className="mt-0.5 text-xs text-muted-foreground">
                    {phaseLabel(run.phase, t)}
                    {running ? "…" : ""}
                  </p>
                </div>
                <button
                  type="button"
                  onClick={() => setPanelOpen(false)}
                  className="rounded-md p-1 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                  aria-label={t("runToast.close")}
                >
                  <X className="h-4 w-4" />
                </button>
              </div>

              <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-muted">
                <div
                  className={cn(
                    "h-full rounded-full bg-gradient-to-r from-primary to-fuchsia-500 transition-all duration-700",
                    running && "animate-pulse",
                  )}
                  style={{ width: `${progress}%` }}
                />
              </div>

              <div className="mt-2.5 flex items-center justify-between text-[11px] font-medium text-muted-foreground">
                <span>{progress}%</span>
                <span>
                  {run.found_count} found · {run.new_count} new
                </span>
              </div>

              <div className="mt-3 flex items-center justify-between gap-2">
                <Link
                  href="/runs"
                  className="text-xs font-semibold text-primary hover:underline"
                  onClick={() => setPanelOpen(false)}
                >
                  {t("runToast.view")}
                </Link>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 px-2 text-xs"
                  onClick={() => setPanelOpen(false)}
                >
                  {t("runToast.close")}
                </Button>
              </div>
            </div>
          </div>
        </div>
      ) : null}
    </>
  );
}
